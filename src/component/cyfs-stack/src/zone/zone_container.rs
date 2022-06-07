use cyfs_base::*;
use cyfs_core::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;

use std::collections::{hash_map::Entry, HashMap};
use std::convert::TryInto;
use std::sync::Arc;

struct ZoneContainerInner {
    // 当前管理的所有zone
    zone_list: HashMap<ZoneId, Zone>,

    // owner到zone的映射管理
    zone_indexer: HashMap<ObjectId, ZoneId>,

    // device到zone的映射管理，一个device只属于一个zone
    zone_device_indexer: HashMap<DeviceId, ZoneId>,
}

impl ZoneContainerInner {
    pub fn new() -> Self {
        Self {
            zone_list: HashMap::new(),
            zone_indexer: HashMap::new(),
            zone_device_indexer: HashMap::new(),
        }
    }

    pub fn on_new_zone(&mut self, zone_id: ZoneId, zone: Zone) {
        // 建立owner到zone的映射
        self.build_index(zone.owner(), &zone_id);

        // 给zone里面的所有device建立索引
        self.build_device_index_for_zone(&zone_id, &zone);

        // 保存zone
        match self.zone_list.entry(zone_id.clone()) {
            Entry::Vacant(entry) => {
                debug!("new zone: zone={}, ood={}", zone_id, zone.ood());
                entry.insert(zone);
            }
            Entry::Occupied(mut entry) => {
                error!(
                    "will replace zone: zoo={}, ood={}->{}",
                    zone_id,
                    entry.get().ood(),
                    zone.ood()
                );

                entry.insert(zone);
            }
        };
    }

    fn build_index(&mut self, owner: &ObjectId, zone_id: &ZoneId) {
        match self.zone_indexer.entry(owner.clone()) {
            Entry::Vacant(entry) => {
                debug!("new owner->zone: {} -> {}", owner, zone_id);

                entry.insert(zone_id.clone());
            }
            Entry::Occupied(mut entry) => {
                warn!(
                    "will replace owner->zone: {}, zone {}->{}",
                    owner,
                    entry.get(),
                    zone_id
                );

                entry.insert(zone_id.clone());
            }
        };
    }

    fn build_device_index_for_zone(&mut self, zone_id: &ZoneId, zone: &Zone) {
        // 给zone里面的所有device建立索引
        for device_id in zone.ood_list() {
            self.build_device_index(device_id, zone_id);
        }

        for device_id in zone.known_device_list() {
            self.build_device_index(device_id, zone_id);
        }
    }

    fn build_device_index(&mut self, device_id: &DeviceId, zone_id: &ZoneId) {
        match self.zone_device_indexer.entry(device_id.clone()) {
            Entry::Vacant(entry) => {
                debug!("new device->zone: {} -> {}", device_id, zone_id);

                entry.insert(zone_id.clone());
            }
            Entry::Occupied(mut entry) => {
                warn!(
                    "will replace device->zone: {}, zone {}->{}",
                    device_id,
                    entry.get(),
                    zone_id
                );

                entry.insert(zone_id.clone());
            }
        };
    }

    pub fn get_or_create_zone_by_owner(
        &mut self,
        owner: ObjectId,
        ood_work_mode: OODWorkMode,
        ood_list: Vec<DeviceId>,
        device_id: Option<&DeviceId>,
    ) -> (ZoneId, Zone, bool) {
        let mut update_noc = false;
        let (zone_id, zone) = {
            // 首先查找owner对应的zone是不是已经存在了
            if let Some(zone_id) = self.zone_indexer.get(&owner) {
                let zone_id = zone_id.clone();
                let mut zone = self.zone_list.get_mut(&zone_id).unwrap().clone();

                // check if ood_work_mode changed
                if *zone.ood_work_mode() != ood_work_mode {
                    zone.set_ood_work_mode(ood_work_mode);
                    update_noc = true;
                }

                // 已经存在zone了，那么尝试更新device_id到zone列表，并保存zone，更新索引
                if let Some(device_id) = device_id {
                    if Self::update_zone_known_device(&zone_id, &mut zone, device_id) {
                        // 保存zone
                        self.zone_list.insert(zone_id.clone(), zone.clone());
                        // 添加device索引
                        self.zone_device_indexer
                            .insert(device_id.clone(), zone_id.clone());
                        update_noc = true;
                    }
                }
                (zone_id, zone)
            } else {
                let mut known_device_list = Vec::new();
                if let Some(device_id) = device_id {
                    if ood_list.iter().find(|&v| v == device_id).is_none() {
                        known_device_list.push(device_id.to_owned());
                    }
                }

                let zone =
                    Zone::create(owner.to_owned(), ood_work_mode, ood_list, known_device_list);
                let zone_id: ZoneId = zone.desc().calculate_id().try_into().unwrap();

                info!(
                    "will create new zone for owner={}, device={:?}, zone={}",
                    owner, device_id, zone_id
                );

                if let Some(_old) = self.zone_list.insert(zone_id.clone(), zone.clone()) {
                    error!(
                        "create new zone but old already exists! zone_id={}",
                        zone_id
                    );
                    unreachable!();
                }

                // 托管zone
                self.zone_indexer.insert(owner.to_owned(), zone_id.clone());

                // 更新device索引
                self.build_device_index_for_zone(&zone_id, &zone);

                update_noc = true;

                (zone_id, zone)
            }
        };

        (zone_id, zone, update_noc)
    }

    // 更新一个zone的known_device列表，如果该device_id不是ood，并且不存在的话
    fn update_zone_known_device(zone_id: &ZoneId, zone: &mut Zone, device_id: &DeviceId) -> bool {
        if zone.is_ood(device_id) {
            return false;
        }

        let known_device_list = zone.known_device_list();
        for known_device_id in known_device_list {
            if known_device_id == device_id {
                return false;
            }
        }

        info!(
            "will update zone device list: zone={}, device={}",
            zone_id, device_id
        );

        zone.known_device_list_mut().push(device_id.clone());

        zone.body_mut()
            .as_mut()
            .unwrap()
            .increase_update_time(bucky_time_now());

        true
    }
}

#[derive(Clone)]
pub struct ZoneContainer {
    device_id: DeviceId,
    noc: Arc<Box<dyn NamedObjectCache>>,
    inner: Arc<Mutex<ZoneContainerInner>>,
}

impl ZoneContainer {
    pub fn new(device_id: DeviceId, noc: Arc<Box<dyn NamedObjectCache>>) -> Self {
        Self {
            device_id,
            noc,
            inner: Arc::new(Mutex::new(ZoneContainerInner::new())),
        }
    }

    pub async fn get_or_create_zone_by_owner(
        &self,
        owner: ObjectId,
        ood_work_mode: OODWorkMode,
        ood_list: Vec<DeviceId>,
        device_id: Option<&DeviceId>,
    ) -> (ZoneId, Zone, bool) {
        let (zone_id, zone, changed) = {
            let mut zone_container = self.inner.lock().unwrap();
            zone_container.get_or_create_zone_by_owner(owner, ood_work_mode, ood_list, device_id)
        };

        if changed {
            self.update_noc(&zone_id, &zone).await;
        }

        (zone_id, zone, changed)
    }

    pub fn query(&self, zone_id: &ZoneId) -> Option<Zone> {
        let zone_container = self.inner.lock().unwrap();

        zone_container
            .zone_list
            .get(zone_id)
            .map(|zone| zone.clone())
    }

    pub fn get_zone_by_owner(&self, owner: &ObjectId) -> Option<Zone> {
        let zone_container = self.inner.lock().unwrap();
        if let Some(zone_id) = zone_container.zone_indexer.get(owner) {
            let zone = zone_container.zone_list.get(zone_id).unwrap().clone();
            return Some(zone);
        }

        None
    }

    pub fn get_zone(&self, device_id: &DeviceId) -> Option<Zone> {
        let zone_container = self.inner.lock().unwrap();
        if let Some(zone_id) = zone_container.zone_device_indexer.get(device_id) {
            match zone_container.zone_list.get(zone_id) {
                Some(zone) => Some(zone.to_owned()),
                None => {
                    warn!(
                        "get zone by device but not exists! device={}, zone={}",
                        device_id, zone_id
                    );
                    None
                }
            }
        } else {
            None
        }
    }

    pub fn get_zone_id(&self, device_id: &DeviceId) -> Option<ZoneId> {
        let zone_container = self.inner.lock().unwrap();
        if let Some(zone_id) = zone_container.zone_device_indexer.get(device_id) {
            return Some(zone_id.clone());
        }

        None
    }

    pub async fn remove_zone_by_device(&self, device_id: &DeviceId) {
        let zone_container = self.inner.lock().unwrap();
        if let Some(zone_id) = zone_container.zone_device_indexer.get(device_id) {
            self.remove_zone(zone_id).await;
        }
    }

    pub async fn remove_zone(&self, zone_id: &ZoneId) {
        let ret = {
            let mut zone_container = self.inner.lock().unwrap();
            zone_container.zone_list.remove(zone_id)
        };

        if let Some(zone) = ret {
            info!("remove zone: zone_id={}, owner={}", zone_id, zone.owner());

            self.remove_from_noc(zone_id).await;
        }
    }

    pub async fn load_from_noc(&self) -> BuckyResult<()> {
        // 过滤noc里面管理的所有的zone对象
        let mut filter = NamedObjectCacheSelectObjectFilter::default();
        filter.obj_type = Some(CoreObjectType::Zone.into());

        let mut opt = NamedObjectCacheSelectObjectOption {
            page_size: 128,
            page_index: 0,
        };

        loop {
            let noc_req = NamedObjectCacheSelectObjectRequest {
                protocol: NONProtocol::Native,
                source: self.device_id.clone(),
                filter: filter.clone(),
                opt: Some(opt.clone()),
            };
            let obj_list = self.noc.select_object(&noc_req).await.map_err(|e| {
                error!("load zone objects from noc failed! {}", e);
                e
            })?;
            let ret_count = obj_list.len();

            let mut zone_container = self.inner.lock().unwrap();
            for obj_info in obj_list {
                let buf = obj_info.object_raw.unwrap();
                let zone_id = obj_info.object_id.try_into().unwrap();
                match Zone::raw_decode(&buf) {
                    Ok((zone, _)) => {
                        zone_container.on_new_zone(zone_id, zone);
                    }
                    Err(e) => {
                        error!("decode zone object error: zone={}, {}", zone_id, e);
                    }
                }
            }

            // 尝试继续下一页的查询
            if ret_count < opt.page_size as usize {
                break;
            }
            opt.page_index += 1;
        }
        Ok(())
    }

    // 保存zone对象到本地noc
    async fn update_noc(&self, zone_id: &ZoneId, zone: &Zone) {
        let object_raw = zone.to_vec().unwrap();
        let (object, _) = AnyNamedObject::raw_decode(&object_raw).unwrap();

        let info = NamedObjectCacheInsertObjectRequest {
            protocol: NONProtocol::Native,
            source: self.device_id.clone(),
            object_id: zone_id.object_id().clone(),
            dec_id: None,
            object_raw,
            object: Arc::new(object),
            flags: 0u32,
        };

        match self.noc.insert_object(&info).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCacheInsertResult::Accept
                    | NamedObjectCacheInsertResult::Updated => {
                        info!("insert zone to noc success! zone={}", zone_id);
                    }
                    r @ _ => {
                        // 不应该到这里？因为zone修改后的update_time已经会被更新
                        // FIXME 如果触发了本地时间回滚之类的问题，这里是否需要强制delete然后再插入？
                        error!(
                            "update zone to noc but alreay exist! zone={}, result={:?}",
                            zone_id, r
                        )
                    }
                }
            }
            Err(e) => {
                error!("insert zone to noc error! zone={}, {}", zone_id, e);
            }
        }
    }

    async fn remove_from_noc(&self, zone_id: &ZoneId) {
        let info = NamedObjectCacheDeleteObjectRequest {
            protocol: NONProtocol::Native,
            source: self.device_id.clone(),
            object_id: zone_id.object_id().clone(),
            flags: 0u32,
        };

        match self.noc.delete_object(&info).await {
            Ok(resp) => {
                if resp.deleted_count > 0 {
                    info!("delete zone from noc success! zone={}", zone_id);
                } else {
                    // 不应该到这里？因为zone修改后的update_time已经会被更新
                    // FIXME 如果触发了本地时间回滚之类的问题，这里是否需要强制delete然后再插入？
                    error!("delete zone from noc but not found! zone={}", zone_id)
                }
            }

            Err(e) => {
                error!("insert zone to noc error! zone={}, {}", zone_id, e);
            }
        }
    }
}
