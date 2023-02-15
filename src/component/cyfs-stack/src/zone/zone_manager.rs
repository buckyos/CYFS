use super::target_zone::TargetZoneManager;
use super::zone_container::ZoneContainer;
use super::{failed_cache::ZoneFailedCache, friends::FriendsManager};
use crate::meta::*;
use crate::resolver::DeviceCache;
use cyfs_base::*;
use cyfs_core::{Zone, ZoneId, ZoneObj};
use cyfs_debug::Mutex;
use cyfs_lib::*;
use cyfs_util::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;

// zone发生改变
pub type FnZoneChanged = dyn EventListenerAsyncRoutine<ZoneId, ()>;
type ZoneChangeEventManager = SyncEventManagerSync<ZoneId, ()>;

pub struct CurrentZoneInfo {
    // 当前设备id
    pub device_id: DeviceId,
    pub device_category: DeviceCategory,

    pub zone_device_ood_id: DeviceId,
    pub zone_id: ZoneId,
    pub zone_role: ZoneRole,
    pub ood_work_mode: OODWorkMode,

    pub owner_id: ObjectId,

    // current zone's owner object, maybe changed on ood modify
    pub owner: Arc<AnyNamedObject>,
}

impl std::fmt::Display for CurrentZoneInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "device={},category={},ood={},zone={},zone_role={},ood_work_mode={},owner={},owner_updatetime={}",
            self.device_id,
            self.device_category,
            self.zone_device_ood_id,
            self.zone_id,
            self.zone_role,
            self.ood_work_mode,
            self.owner_id,
            self.owner.get_update_time(),
        )
    }
}

pub type CurrentZoneInfoRef = Arc<CurrentZoneInfo>;

#[derive(Clone)]
pub struct ZoneManager {
    noc: NamedObjectCacheRef,
    device_manager: Arc<Box<dyn DeviceCache>>,
    device_id: DeviceId,
    device_category: DeviceCategory,

    // managed all zones
    zones: ZoneContainer,

    // 当前的zone信息
    current_info: Arc<Mutex<Option<Arc<CurrentZoneInfo>>>>,

    meta_cache: Arc<MetaCacheRef>,

    zone_changed_event: ZoneChangeEventManager,

    failed_cache: ZoneFailedCache,

    fail_handler: ObjectFailHandler,

    friends_manager: FriendsManager,

    search_zone_reenter_call_manager: ReenterCallManager<DeviceId, BuckyResult<Zone>>,
    search_zone_ood_by_owner_reenter_call_manager:
        ReenterCallManager<ObjectId, BuckyResult<(ObjectId, OODWorkMode, Vec<DeviceId>)>>,

    target_zone_manager: Arc<OnceCell<TargetZoneManager>>,
}

pub type ZoneManagerRef = Arc<ZoneManager>;

impl ZoneManager {
    pub fn new(
        noc: NamedObjectCacheRef,
        device_manager: Box<dyn DeviceCache>,
        device_id: DeviceId,
        device_category: DeviceCategory,
        meta_cache: MetaCacheRef,
        fail_handler: ObjectFailHandler,
        root_state: GlobalStateOutputProcessorRef,
        local_cache: GlobalStateOutputProcessorRef,
    ) -> ZoneManagerRef {
        let device_manager = Arc::new(device_manager);
        let meta_cache = Arc::new(meta_cache);

        let ret = Self {
            noc: noc.clone(),
            device_manager,
            device_id: device_id.clone(),
            device_category,
            zones: ZoneContainer::new(device_id, local_cache, noc),
            current_info: Arc::new(Mutex::new(None)),
            meta_cache,
            zone_changed_event: ZoneChangeEventManager::new(),
            failed_cache: ZoneFailedCache::new(),
            fail_handler,
            search_zone_reenter_call_manager: ReenterCallManager::new(),
            search_zone_ood_by_owner_reenter_call_manager: ReenterCallManager::new(),
            friends_manager: FriendsManager::new(root_state),
            target_zone_manager: Arc::new(OnceCell::new()),
        };

        let ret = Arc::new(ret);

        let target_zone_manager = TargetZoneManager::new(ret.clone());
        if let Err(_) = ret.target_zone_manager.set(target_zone_manager) {
            unreachable!();
        }

        ret
    }

    pub fn device_manager(&self) -> &Box<dyn DeviceCache> {
        &self.device_manager
    }

    pub fn zone_changed_event(&self) -> &ZoneChangeEventManager {
        &self.zone_changed_event
    }

    pub fn target_zone_manager(&self) -> &TargetZoneManager {
        self.target_zone_manager.get().unwrap()
    }

    pub async fn init(&self) -> BuckyResult<()> {
        self.friends_manager.init().await?;

        self.zones.load_from_noc().await?;

        Ok(())
    }

    // check if zone info match with its owner, return true if matched
    fn compare_zone_with_owner(zone: &Zone, owner: &AnyNamedObject) -> BuckyResult<bool> {
        let ood_list = owner.ood_list()?;
        if zone.ood_list() != ood_list {
            warn!(
                "zone ood_list changed! zone={:?}, owner={:?}",
                zone.ood_list(),
                ood_list
            );
            return Ok(false);
        }

        let ood_work_mode = owner.ood_work_mode()?;
        if *zone.ood_work_mode() != ood_work_mode {
            warn!(
                "zone ood_work_mode changed! zone={:?}, owner={:?}",
                zone.ood_work_mode(),
                ood_work_mode
            );
            return Ok(false);
        }

        Ok(true)
    }

    // 获取当前协议栈的zone信息
    pub async fn get_current_info(&self) -> BuckyResult<Arc<CurrentZoneInfo>> {
        // current_info只需要初始化一次即可
        let current_info = self.current_info.lock().unwrap().clone();
        if current_info.is_none() {
            let mut zone = self.get_zone(&self.device_id, None).await?;
            let zone_id = zone.zone_id();

            // load current zone's owner
            let owner_id = zone.owner().to_owned();
            let owner = self.search_object(&owner_id).await?;

            info!("current zone owner: {}", owner.format_json().to_string());

            // verify if owner object changed
            if let Ok(false) = Self::compare_zone_with_owner(&zone, &owner) {
                self.remove_zone(&zone_id).await;
                zone = self.get_zone(&self.device_id, None).await?;
            }

            let zone_device_ood_id = zone.ood().to_owned();

            let ood_work_mode = zone.ood_work_mode().to_owned();
            let zone_role = if self.device_id == zone_device_ood_id {
                ZoneRole::ActiveOOD
            } else if zone.is_ood(&self.device_id) {
                match ood_work_mode {
                    OODWorkMode::Standalone => ZoneRole::ReservedOOD,
                    OODWorkMode::ActiveStandby => ZoneRole::StandbyOOD,
                }
            } else {
                ZoneRole::Device
            };

            let info = CurrentZoneInfo {
                device_id: self.device_id.clone(),
                device_category: self.device_category.clone(),
                zone_device_ood_id,
                zone_id,
                zone_role,
                ood_work_mode,
                owner_id,
                owner: Arc::new(owner),
            };

            info!("current zone info: {}", info,);

            let info = Arc::new(info);
            {
                let mut slot = self.current_info.lock().unwrap();
                *slot = Some(info.clone());
            }
            Ok(info)
        } else {
            Ok(current_info.unwrap())
        }
    }

    pub fn get_current_device_id(&self) -> &DeviceId {
        &self.device_id
    }

    pub async fn get_current_zone(&self) -> BuckyResult<Zone> {
        self.get_zone(&self.device_id, None).await
    }

    pub async fn get_current_zone_id(&self) -> BuckyResult<ZoneId> {
        self.get_zone_id(&self.device_id, None).await
    }

    pub fn query(&self, zone_id: &ZoneId) -> Option<Zone> {
        if let Some(zone) = self.zones.query(zone_id) {
            return Some(zone);
        }

        // 需要在孤儿zone里面查找
        self.failed_cache.query_zone(zone_id)
    }

    pub async fn get_zone(
        &self,
        device_id: &DeviceId,
        device: Option<Device>,
    ) -> BuckyResult<Zone> {
        // 首先本地查询
        if let Some(zone) = self.zones.get_zone(device_id) {
            return Ok(zone);
        }

        // 检查是不是存在对应的孤儿zone，如果存在，则不再尝试创建新的zone
        if let Some(zone) = self.failed_cache.get_orphan_zone(device_id) {
            warn!("get orphan zone: device={}", device_id);
            return Ok(zone);
        }

        // 进入发现流程... 同一个device_id同一时刻只能发起一次search操作
        let this = self.clone();
        let owned_device_id = device_id.to_owned();
        self.search_zone_reenter_call_manager
            .call(&device_id, async move {
                match this.search_zone(&owned_device_id, device).await {
                    Ok(zone) => Ok(zone),
                    Err(e) => {
                        // 如果是当前协议栈，那么不允许创建孤儿zone，直接返回错误
                        if owned_device_id == this.device_id {
                            error!(
                                "current stack's device not support orphan zone! device={}, {}",
                                owned_device_id, e
                            );
                            return Err(e);
                        }

                        Ok(this.failed_cache.get_orphan_zone(&owned_device_id).unwrap())
                    }
                }
            })
            .await
    }

    pub async fn get_zone_id(
        &self,
        device_id: &DeviceId,
        device: Option<Device>,
    ) -> BuckyResult<ZoneId> {
        let zone = self.get_zone(device_id, device).await?;
        Ok(zone.zone_id())
    }

    pub async fn get_zone_by_owner(
        &self,
        owner_id: &ObjectId,
        object: Option<AnyNamedObject>,
    ) -> BuckyResult<Zone> {
        // 先查找本地是否已经存在该owner对应的zone
        if let Some(zone) = self.zones.get_zone_by_owner(owner_id) {
            return Ok(zone);
        }

        // 检查是不是存在对应的错误缓存，如果存在，则不再尝试创建新的zone，直接失败
        if let Some(e) = self.failed_cache.get_failed_owner(owner_id) {
            warn!("zone owner still in error cache! owner={}", owner_id);
            return Err(e);
        }

        // 先查找owner的ood列表
        // 为了避免同一个owner_id并发发起操作，这里需要增加一层放重入保证
        let this = self.clone();
        let mut mut_owner_id = owner_id.to_owned();
        let (owner_id, ood_work_mode, ood_list) = self
            .search_zone_ood_by_owner_reenter_call_manager
            .call(owner_id, async move {
                let (ood_work_mode, ood_list) = this
                    .search_zone_ood_by_owner(&mut mut_owner_id, object)
                    .await
                    .map_err(|e| {
                        // 查找owner失败后，需要缓存失败的结果，避免频繁发起查询
                        this.failed_cache.on_owner_failed(&mut_owner_id, e.clone());
                        e
                    })?;

                Ok((mut_owner_id, ood_work_mode, ood_list))
            })
            .await?;

        // 使用device索引做一次本地查询
        for ood_device_id in &ood_list {
            if let Some(zone) = self.zones.get_zone(&ood_device_id) {
                return Ok(zone);
            }
        }

        // 获取这个ood的zone，不存在的话会创建
        let zone = self
            .get_or_create_zone_by_owner(owner_id, ood_work_mode, ood_list, None)
            .await;
        Ok(zone)
    }

    pub async fn get_zone_id_by_owner(
        &self,
        owner_id: &ObjectId,
        object: Option<AnyNamedObject>,
    ) -> BuckyResult<ZoneId> {
        self.get_zone_by_owner(owner_id, object)
            .await
            .map(|zone| zone.zone_id())
    }

    // 根据一个device,判断其zone方向
    pub async fn get_zone_direction(
        &self,
        device_id: &DeviceId,
        device: Option<Device>,
        incoming: bool,
    ) -> BuckyResult<ZoneDirection> {
        let zone_id = self.get_zone_id(device_id, device).await?;
        let current_zone_id = self.get_current_zone_id().await?;

        let ret = if zone_id == current_zone_id {
            ZoneDirection::LocalToLocal
        } else {
            match incoming {
                true => ZoneDirection::RemoteToLocal,
                false => ZoneDirection::LocalToRemote,
            }
        };

        Ok(ret)
    }

    pub fn get_zone_ood(&self, zone_id: &ZoneId) -> BuckyResult<DeviceId> {
        match self.query(zone_id) {
            Some(zone) => Ok(zone.ood().clone()),
            None => {
                error!("zone not exists: {}", zone_id);
                Err(BuckyError::from(BuckyErrorCode::NotFound))
            }
        }
    }

    // 查找一个设备在zone里面的ood索引， 从0开始，不在ood列表里面则返回MAX
    pub async fn get_device_zone_ood_index(
        &self,
        zone: &Zone,
        device_id: &DeviceId,
    ) -> BuckyResult<usize> {
        self.get_device_zone_ood_index_impl(&zone, device_id).await
    }

    // 查找一个设备在zone里面的ood索引， 从0开始，不在ood列表里面则返回MAX
    async fn get_device_zone_ood_index_impl(
        &self,
        zone: &Zone,
        device_id: &DeviceId,
    ) -> BuckyResult<usize> {
        if zone.ood() == device_id {
            info!("device is zone main ood: {}", device_id);
            return Ok(0);
        }

        let device = self.device_manager.search(zone.ood()).await?;

        match device.desc().owner() {
            Some(owner_id) => {
                let obj_type = owner_id.obj_type_code();

                // 目前owner只支持people和simplegroup
                // People,SimpleGroup对象存在ood_list
                match obj_type {
                    ObjectTypeCode::People | ObjectTypeCode::Group => {
                        // 查找owner对象
                        info!("will search owner: type={:?}, {}", obj_type, owner_id);
                        let object = self.search_object(&owner_id).await?;
                        match object.ood_list() {
                            Ok(list) if list.len() > 0 => {
                                debug!(
                                    "get ood list from owner object ood_list: {} {:?}",
                                    owner_id, list
                                );

                                let obj_id = device_id.object_id();
                                for i in 0..list.len() {
                                    if list[i] == *obj_id {
                                        info!(
                                            "device in ood list: device={}, index={}",
                                            device_id, i
                                        );
                                        return Ok(i);
                                    }
                                }

                                info!("device not in ood list: device={}", device_id);
                                Ok(std::usize::MAX)
                            }
                            _ => {
                                let msg = format!(
                                    "get ood list from object ood_list not found or empty: {}",
                                    owner_id
                                );
                                error!("{}", msg);
                                let _ = self.fail_handler.try_flush_object(&owner_id).await;

                                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                            }
                        }
                    }
                    _ => {
                        let msg = format!(
                            "unsupport owner object type: obj={} type={:?}",
                            owner_id, obj_type
                        );
                        error!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
                    }
                }
            }
            None => {
                if device_id == zone.ood() {
                    Ok(0)
                } else {
                    Ok(std::usize::MAX)
                }
            }
        }
    }

    pub async fn remove_zone_by_device(&self, device_id: &DeviceId) {
        if let Some(zone_id) = self.zones.get_zone_id(device_id) {
            self.remove_zone(&zone_id).await;
        }
    }

    pub async fn remove_zone(&self, zone_id: &ZoneId) {
        self.zones.remove_zone(zone_id).await;

        if let Some(zone) = self.failed_cache.remove_zone(zone_id) {
            info!(
                "remove orphan zone: zone_id={}, owner={}",
                zone_id,
                zone.owner()
            );
        }

        let mut slot = self.current_info.lock().unwrap();
        if let Some(info) = &*slot {
            if info.zone_id == *zone_id {
                drop(info);

                *slot = None;
                info!("clear current zone info! zone_id={}", zone_id);
            }
        }
    }

    async fn search_zone(&self, device_id: &DeviceId, device: Option<Device>) -> BuckyResult<Zone> {
        debug!("will search zone for device: {}", device_id);

        // 找到device的所在zone的ood列表，一个owner关联唯一的zone
        let (owner, ood_work_mode, ood_list) = self
            .search_zone_ood(device_id, device)
            .await
            .map_err(|e| {
                let msg = format!("search zone ood failed! device={}, {}", device_id, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::NotFound, msg)
            })
            .map_err(|e| {
                // device创建zone失败后，尝试创建对应的孤儿zone来使用
                self.failed_cache.on_device_zone_failed(device_id);
                e
            })?;

        let zone = self
            .get_or_create_zone_by_owner(owner, ood_work_mode, ood_list, Some(device_id))
            .await;
        Ok(zone)
    }

    async fn get_or_create_zone_by_owner(
        &self,
        owner: ObjectId,
        ood_work_mode: OODWorkMode,
        ood_list: Vec<DeviceId>,
        device_id: Option<&DeviceId>,
    ) -> Zone {
        let (zone_id, zone, changed) = self
            .zones
            .get_or_create_zone_by_owner(owner, ood_work_mode, ood_list, device_id)
            .await;

        if changed {
            // 触发zone改变事件
            let _r = self.zone_changed_event.emit(&zone_id);
        }

        zone
    }

    // 查找一个设备的所在zone的ood列表
    // 核心流程是查找device的owner，owner的ood_list
    // 目前owner只支持people和simplegroup两种
    // 如果device没有owner，那么owner就是自己，形成一个单device的zone
    // return: owner和当前device_id在ood列表里面的索引，不在里面则返回std::usize::MAX
    async fn search_zone_ood(
        &self,
        device_id: &DeviceId,
        device: Option<Device>,
    ) -> BuckyResult<(ObjectId, OODWorkMode, Vec<DeviceId>)> {
        let device = if device.is_none() {
            self.device_manager.search(device_id).await?
        } else {
            device.unwrap()
        };

        // 校验device签名是否有效，只有有效签名的device才能加入zone
        if let Err(e) = self
            .device_manager
            .verfiy_owner(device_id, Some(&device))
            .await
        {
            error!("verify device owner failed! device={}", device_id);
            return Err(e);
        }

        match device.desc().owner() {
            Some(owner) => {
                // People,SimpleGroup对象存在ood_list
                let mut owner = owner.to_owned();
                let (ood_work_mode, ood_list) =
                    self.search_zone_ood_by_owner(&mut owner, None).await?;
                Ok((owner, ood_work_mode, ood_list))
            }
            None => match device.category() {
                Ok(DeviceCategory::OOD) => {
                    info!("ood device without owner: device={}", device_id);
                    Ok((
                        device_id.object_id().to_owned(),
                        OODWorkMode::Standalone,
                        vec![device_id.clone()],
                    ))
                }
                _ => {
                    let msg = format!(
                        "device category not specified or invalid: device={} category={:?}",
                        device_id,
                        device.category()
                    );
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                }
            },
        }
    }

    /*
    async fn verify_owner_body(obj: &AnyNamedObject) -> BuckyResult<()> {
        let pk = obj.public_key();
        if pk.is_none() {
            let msg = format!("zone owner has no publicKey! owner={}", obj.object_id());
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidSignature, msg));
        }

        use cyfs_lib::*;
        let req = VerifyByObjectRequest {

        }
        let verifier = RsaCPUObjectVerifier::new(pk.)
    }
    */

    // 查找一个owner的所在zone的ood列表
    // 核心流程是向上查找device的owner，直到people/simplegroup, owner的ood_list的第一个ood device
    // 目前owner只支持people和simplegroup两种
    pub(super) async fn search_zone_ood_by_owner(
        &self,
        owner_id: &mut ObjectId,
        mut object: Option<AnyNamedObject>,
    ) -> BuckyResult<(OODWorkMode, Vec<DeviceId>)> {
        loop {
            let obj_type = owner_id.obj_type_code();

            // 查找owner对象
            let owner_object = if object.is_none() {
                info!("will search owner: type={:?}, {}", obj_type, owner_id);

                self.search_object(&owner_id).await?
            } else {
                object.unwrap()
            };

            // 目前owner只支持people和simplegroup
            // People,SimpleGroup对象存在ood_list
            if obj_type == ObjectTypeCode::People || obj_type == ObjectTypeCode::Group {
                match owner_object.ood_list() {
                    Ok(list) => {
                        if list.len() > 0 {
                            let work_mode = owner_object.ood_work_mode().unwrap().to_owned();
                            debug!(
                                "get ood list from owner object ood_list: owner={}, work_mode={:?}, list={:?}",
                                owner_id, work_mode, list
                            );

                            return Ok((work_mode, list.to_owned()));
                            /*
                            let ood_device_id = list[0].clone();
                            let obj_type = ood_device_id.object_id().obj_type_code();
                            if obj_type == ObjectTypeCode::Device {
                                info!(
                                    "search device's owner ood: owner={}, ood={}",
                                    owner_id, ood_device_id
                                );
                                break Ok(ood_device_id);
                            } else {
                                let msg = format!(
                                    "ood not valid device type: ood={} obj_type={:?}",
                                    ood_device_id, obj_type
                                );
                                error!("{}", msg);
                                break Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                            }
                            */
                        } else {
                            let msg =
                                format!("get ood list from people/simple_group ood_list empty: owner={}, owner_type={:?}", owner_id, obj_type);
                            error!("{}", msg);

                            // 尝试刷新一下对象，如果更新了，那么需要再次尝试
                            match self.fail_handler.try_flush_object(&owner_id).await {
                                Ok(ret) => {
                                    if ret {
                                        object = None;
                                        continue;
                                    }
                                }
                                Err(_) => {}
                            };

                            break Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                        }
                    }
                    Err(e) => {
                        // 如果没指定ood_list，由于又不存在owner，终止查找
                        warn!(
                            "get ood_list from owner object failed! owner={} {}",
                            owner_id, e
                        );
                        break Err(e);
                    }
                }
            } else {
                // 继续向上查找一级owner
                *owner_id = match owner_object.owner() {
                    Some(owner_id) => owner_id.clone(),
                    None => {
                        let msg = format!(
                            "unsupport owner object type: obj={} type={:?}",
                            owner_id, obj_type
                        );
                        error!("{}", msg);
                        break Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                    }
                };
                object = None;
            }
        }
    }

    async fn search_object(&self, object_id: &ObjectId) -> BuckyResult<AnyNamedObject> {
        let obj = loop {
            let object_raw = self.search_object_raw(object_id).await?;

            let (obj, _) = AnyNamedObject::raw_decode(&object_raw).map_err(|e| {
                error!(
                    "decode raw data from meta chain failed! obj={} err={}",
                    object_id, e
                );
                e
            })?;

            match object_id.obj_type_code() {
                ObjectTypeCode::People | ObjectTypeCode::Group => {
                    // 需要校验签名
                    let obj = Arc::new(obj);

                    match self.device_manager.verfiy_own_signs(object_id, &obj).await {
                        Ok(_) => {
                            break Arc::try_unwrap(obj).unwrap();
                        }
                        Err(e) => match self.fail_handler.try_flush_object(object_id).await {
                            Err(_) => return Err(e),
                            Ok(changed) => {
                                if changed {
                                    // object updated from meta, now will reload object and reverify signs
                                    continue;
                                } else {
                                    return Err(e);
                                }
                            }
                        },
                    }
                }
                _ => break obj,
            }
        };

        Ok(obj)
    }

    // 查找一个owner对象，先从本地查找，再从meta-chain查找
    async fn search_object_raw(&self, object_id: &ObjectId) -> BuckyResult<Vec<u8>> {
        let req = NamedObjectCacheGetObjectRequest {
            object_id: object_id.clone(),
            source: RequestSourceInfo::new_local_system(),
            last_access_rpath: None,
        };
        if let Ok(Some(obj)) = self.noc.get_object(&req).await {
            debug!("get object from noc: {}", object_id);
            return Ok(obj.object.object_raw);
        }

        // 从meta查询
        match self.meta_cache.get_object(object_id).await? {
            Some(data) => {
                debug!("get object from meta: {}", object_id);
                Ok(data.object_raw)
            }
            None => {
                let msg = format!("object not found from meta: {}", object_id);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }

    // 从device/people/simplegroup获取zone
    pub async fn resolve_zone(
        &self,
        object_id: &ObjectId,
        object_raw: Option<Vec<u8>>,
    ) -> BuckyResult<Zone> {
        let type_code = object_id.obj_type_code();
        if type_code == ObjectTypeCode::Device {
            let device_id = object_id.try_into().unwrap();
            let device = match object_raw {
                Some(buf) => match Device::raw_decode(&buf) {
                    Ok((device, _)) => Some(device),
                    Err(e) => {
                        let msg = format!("decode device error: err={}", e);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                },
                None => None,
            };

            self.get_zone(&device_id, device).await
        } else {
            let object = match object_raw {
                Some(buf) => match AnyNamedObject::raw_decode(&buf) {
                    Ok((object, _)) => Some(object),
                    Err(e) => {
                        let msg = format!("decode object error: err={}", e);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                },
                None => None,
            };

            self.get_zone_by_owner(&object_id, object).await
        }
    }

    pub async fn get_current_source_info(
        &self,
        dec: &Option<ObjectId>,
    ) -> BuckyResult<RequestSourceInfo> {
        let current_info = self.get_current_info().await?;
        let mut ret = RequestSourceInfo::new_local_dec(dec.to_owned());
        ret.zone.zone = Some(current_info.owner_id.clone());
        ret.zone.device = Some(current_info.device_id.clone());

        Ok(ret)
    }

    pub async fn resolve_source_info(
        &self,
        dec: &Option<ObjectId>,
        source: DeviceId,
    ) -> BuckyResult<RequestSourceInfo> {
        let mut ret = loop {
            let current_info = self.get_current_info().await?;
            if source == self.device_id {
                let mut ret = RequestSourceInfo::new_local_dec(dec.to_owned());
                ret.zone.zone = Some(current_info.owner_id.clone());
                break ret;
            }

            let current_zone = self.get_current_zone().await?;
            if current_zone.is_known_device(&source) {
                let mut ret = RequestSourceInfo::new_zone_dec(dec.to_owned());
                ret.zone.zone = Some(current_info.owner_id.clone());
                break ret;
            }

            let zone = self.zones.get_zone(&source);
            if let Some(zone) = zone {
                let mut ret = if self.friends_manager.is_friend(zone.owner()) {
                    RequestSourceInfo::new_friend_zone_dec(dec.to_owned())
                } else {
                    RequestSourceInfo::new_other_zone_dec(dec.to_owned())
                };

                ret.zone.zone = Some(zone.owner().clone());
                break ret;
            }

            // get device to check owner if friend
            let ret = self.device_manager.get(&source).await;
            if let Some(device) = ret {
                let owner = match device.desc().owner().as_ref() {
                    Some(id) => id.to_owned(),
                    None => {
                        warn!(
                            "source device has not owner, now will treat as orphan zone! {}",
                            source
                        );
                        source.object_id().to_owned()
                    }
                };

                // Need resolve zone if device's owner is current zone'owner or friends zpne's owner
                if owner == current_info.owner_id || self.friends_manager.is_friend(&owner) {
                    // need resolve zone!
                    match self.get_zone(&source, Some(device)).await {
                        Ok(zone) => {
                            assert!(zone.is_known_device(&source));
                            if owner == current_info.owner_id {
                                let mut ret = RequestSourceInfo::new_zone_dec(dec.to_owned());
                                ret.zone.zone = Some(current_info.owner_id.clone());
                                break ret;
                            } else {
                                let mut ret =
                                    RequestSourceInfo::new_friend_zone_dec(dec.to_owned());
                                ret.zone.zone = Some(zone.owner().clone());
                                break ret;
                            }
                        }
                        Err(e) => {
                            // FIXME add black list to block the error friend requests
                            error!(
                                "resolve friend zone from source but failed! source={}, {}",
                                source, e
                            );
                            let mut ret = RequestSourceInfo::new_other_zone_dec(dec.to_owned());
                            ret.zone.zone = Some(owner);
                            break ret;
                        }
                    }
                } else {
                    let mut ret = RequestSourceInfo::new_other_zone_dec(dec.to_owned());
                    ret.zone.zone = Some(owner.to_owned());
                    break ret;
                }
            } else {
                warn!(
                    "get device from local but not found! now will treat as other zone! {}",
                    source
                );
                let mut ret = RequestSourceInfo::new_other_zone_dec(dec.to_owned());
                ret.zone.zone = Some(source.object_id().to_owned());
                break ret;
            }
        };

        ret.zone.device = Some(source);

        Ok(ret)
    }
}
