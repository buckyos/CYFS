use super::obj_searcher::*;
use crate::crypto_api::*;
use cyfs_base::*;
use cyfs_bdt::DeviceCache;
use cyfs_lib::*;

use async_trait::async_trait;
use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, RwLock};

pub(crate) struct DeviceInfoManagerImpl {
    noc: NamedObjectCacheRef,

    obj_verifier: Arc<ObjectVerifier>,

    // 本地device
    local_device_id: DeviceId,
    local_device: RwLock<Device>,

    // 内存缓存
    list: RwLock<HashMap<DeviceId, Device>>,

    // used for search from meta chain
    obj_searcher: ObjectSearcherRef,
}

impl DeviceInfoManagerImpl {
    pub fn new(
        noc: NamedObjectCacheRef,
        obj_verifier: Arc<ObjectVerifier>,
        local_device: Device,
        obj_searcher: ObjectSearcherRef,
    ) -> Self {
        let local_device_id = local_device.desc().device_id();
        Self {
            noc,
            obj_verifier,
            local_device_id,
            obj_searcher,
            local_device: RwLock::new(local_device),
            list: RwLock::new(HashMap::new()),
        }
    }

    pub fn local_device_id(&self) -> &DeviceId {
        &self.local_device_id
    }

    pub fn local_device(&self) -> Device {
        let local = self.local_device.read().unwrap();
        (&*local).clone()
    }

    pub fn update_local_device(&self, desc: &Device) {
        let mut local = self.local_device.write().unwrap();
        *local = desc.clone();
    }

    // 从内存和本地noc查找
    pub async fn get_device(&self, device_id: &DeviceId) -> Option<Device> {
        if let Some(device) = self.get_from_memory(device_id) {
            return Some(device);
        }
        if let Ok(Some(device)) = self.get_from_noc(device_id).await {
            self.add_device_to_list(device_id, &device);
            return Some(device);
        }

        None
    }

    // 本地和网络查找
    pub async fn search_device(&self, device_id: &DeviceId) -> BuckyResult<Device> {
        if let Some(device) = self.get_device(device_id).await {
            return Ok(device);
        }
        info!("will search device: {}", device_id);

        self.search_and_save(device_id).await
    }

    fn get_from_memory(&self, device_id: &DeviceId) -> Option<Device> {
        if *device_id == self.local_device_id {
            return Some(self.local_device());
        }

        self.list.read().unwrap().get(device_id).map(|d| d.clone())
    }

    async fn verfiy_own_signs(
        &self,
        object_id: &ObjectId,
        object: &Arc<AnyNamedObject>,
    ) -> BuckyResult<()> {
        let req = VerifyObjectInnerRequest {
            sign_type: VerifySignType::Body,
            object: ObjectInfo {
                object_id: object_id.to_owned(),
                object: object.clone(),
            },
            sign_object: VerifyObjectType::Own,
        };

        match self.obj_verifier.verify_object_inner(req).await {
            Ok(ret) => {
                if ret.valid {
                    info!("verify object own's body sign success! id={}", object_id,);
                    Ok(())
                } else {
                    let msg = format!("verify object own's body sign unmatch! id={}", object_id,);
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::InvalidSignature, msg))
                }
            }
            Err(e) => {
                error!(
                    "verify object own's body sign by owner error! id={}, {}",
                    object_id, e
                );
                Err(e)
            }
        }
    }

    async fn verfiy_owner(&self, device_id: &DeviceId, device: Option<&Device>) -> BuckyResult<()> {
        let d;
        let device = match device {
            Some(v) => v,
            None => {
                d = self.search_device(device_id).await?;
                &d
            }
        };

        if let Some(owner) = device.desc().owner() {
            let object = AnyNamedObject::Standard(StandardObject::Device(device.clone()));
            let object = Arc::new(object);

            let req = VerifyObjectInnerRequest {
                sign_type: VerifySignType::Desc,
                object: ObjectInfo {
                    object_id: device_id.object_id().to_owned(),
                    object,
                },
                sign_object: VerifyObjectType::Owner,
            };

            match self.obj_verifier.verify_object_inner(req).await {
                Ok(ret) => {
                    if ret.valid {
                        info!(
                            "verify device's desc sign by owner success! device={}, owner={}",
                            device_id, owner
                        );
                        Ok(())
                    } else {
                        let msg = format!(
                            "verify device's desc sign by owner unmatch! device={}, owner={}",
                            device_id, owner
                        );
                        error!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::InvalidSignature, msg))
                    }
                }
                Err(e) => {
                    error!(
                        "verify device's desc sign by owner error! device={}, owner={}, {}",
                        device_id, owner, e
                    );
                    Err(e)
                }
            }
        } else {
            warn!("device has no owner! device={}", device_id);
            Ok(())
        }
    }

    pub async fn add_device(&self, device_id: &DeviceId, device: Device) {
        // FIXME 这里添加一个检测，确保添加的device id匹配
        let real_device_id = device.desc().device_id();
        if *device_id != real_device_id {
            let msg = format!(
                "add device but unmatch device_id! param_id={}, calc_id={}",
                device_id, real_device_id
            );
            error!("{}", msg);
            // panic!("{}", msg);
            return;
        }

        if self.add_device_to_list(device_id, &device) {
            let _ = self.update_noc(device_id, device).await;
        }
    }

    fn add_device_to_list(&self, device_id: &DeviceId, device: &Device) -> bool {
        let mut changed = false;
        {
            let mut cache = self.list.write().unwrap();
            match cache.entry(device_id.clone()) {
                Entry::Vacant(v) => {
                    info!("new device in cache: {}", device_id);

                    v.insert(device.clone());
                    changed = true;
                }
                Entry::Occupied(mut o) => {
                    let old_time = o.get().latest_update_time();
                    let new_time = device.latest_update_time();
                    if new_time > old_time {
                        info!(
                            "replace old device in cache: {}, update {} -> {}",
                            device_id, old_time, new_time
                        );
                        o.insert(device.clone());
                        changed = true;
                    }
                }
            }
        }

        changed
    }

    async fn search_and_save(&self, device_id: &DeviceId) -> BuckyResult<Device> {
        let device = self.search(device_id).await?;

        // 保存到缓存
        self.add_device(device_id, device.clone()).await;

        // meta_cache里面会更新noc，所以这里不需要再更细noc了
        // FIXME 这里是否要触发router相关事件逻辑？
        // let _ = self.update_noc(device_id, device.clone()).await;

        Ok(device)
    }

    async fn update_noc(&self, device_id: &DeviceId, device: Device) -> BuckyResult<()> {
        let object_raw = device.to_vec()?;
        let object = AnyNamedObject::Standard(StandardObject::Device(device));

        let object = NONObjectInfo::new(
            device_id.object_id().clone(),
            object_raw,
            Some(Arc::new(object)),
        );

        let info = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            last_access_rpath: None,
            context: None,
            access_string: Some(AccessString::full_except_write().value()),
        };

        match self.noc.put_object(&info).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCachePutObjectResult::AlreadyExists => {
                        debug!("device already in noc: {}", device_id);
                    }
                    NamedObjectCachePutObjectResult::Merged => {
                        info!("device already in noc and signs updated: {}", device_id);
                    }
                    _ => {
                        info!("insert new device to noc success: {}", device_id);
                    }
                }
                Ok(())
            }
            Err(e) => {
                error!("insert device to noc failed: {}", device_id);

                Err(e)
            }
        }
    }

    async fn get_from_noc(&self, device_id: &DeviceId) -> BuckyResult<Option<Device>> {
        let req = NamedObjectCacheGetObjectRequest {
            object_id: device_id.object_id().clone(),
            source: RequestSourceInfo::new_local_system(),
            last_access_rpath: None,
        };

        match self.noc.get_object(&req).await? {
            Some(info) => match Device::raw_decode(&info.object.object_raw) {
                Ok((device, _)) => {
                    debug!("get device object from noc: {}", device_id);
                    Ok(Some(device))
                }
                Err(e) => {
                    error!(
                        "decode device object from noc failed! id={}, {}",
                        device_id, e
                    );
                    Err(e)
                }
            },
            None => {
                info!("get device from noc but not found: {}", device_id);
                Ok(None)
            }
        }
    }

    // return (object, save_to_noc)
    async fn search(&self, device_id: &DeviceId) -> BuckyResult<Device> {
        let mut ret = self
            .obj_searcher
            .search_ex(
                None,
                device_id.object_id(),
                ObjectSearcherFlags::none_local(),
            )
            .await
            .map_err(|e| {
                let msg = format!(
                    "search target device but not found! target={}, {}",
                    device_id, e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::TargetNotFound, msg)
            })?;

        let object = ret.take_object();
        if object.obj_type_code() != ObjectTypeCode::Device {
            let msg = format!(
                "unmatch object type, not device: {}, {:?}",
                device_id,
                object.obj_type_code()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        match Arc::try_unwrap(object) {
            Ok(object) => match object {
                AnyNamedObject::Standard(object) => match object {
                    StandardObject::Device(device) => Ok(device),
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
            Err(_object) => {
                let device = Device::clone_from_slice(&ret.object_raw).map_err(|e| {
                    let msg = format!(
                        "decode device object error: {}, buf={}, {}",
                        device_id,
                        ::hex::encode(&ret.object_raw),
                        e
                    );
                    error!("{}", msg);

                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

                Ok(device)
            }
        }
    }
}

#[derive(Clone)]
pub struct DeviceInfoManager(Arc<DeviceInfoManagerImpl>);

impl DeviceInfoManager {
    pub(crate) fn new(
        noc: NamedObjectCacheRef,
        obj_verifier: Arc<ObjectVerifier>,
        obj_searcher: ObjectSearcherRef,
        local_device: Device,
    ) -> Self {
        let inner = DeviceInfoManagerImpl::new(noc, obj_verifier, local_device, obj_searcher);
        Self(Arc::new(inner))
    }

    pub fn local_device_id(&self) -> &DeviceId {
        self.0.local_device_id()
    }

    pub fn local_device(&self) -> Device {
        self.0.local_device()
    }

    pub fn update_local_device(&self, device: &Device) {
        self.0.update_local_device(device)
    }

    // 本地查找(缓存+noc)
    pub async fn get_device(&self, device_id: &DeviceId) -> Option<Device> {
        self.0.get_device(device_id).await
    }

    // 本地和网络查找
    pub async fn search_device(&self, device_id: &DeviceId) -> BuckyResult<Device> {
        self.0.search_device(device_id).await
    }
}

#[async_trait]
impl DeviceCache for DeviceInfoManager {
    // 添加一个device并保存
    async fn add(&self, device_id: &DeviceId, device: Device) {
        self.0.add_device(device_id, device).await
    }

    // 直接在本地数据查询
    async fn get(&self, device_id: &DeviceId) -> Option<Device> {
        self.get_device(device_id).await
    }

    // 本地查询，查询不到则发起网络查找操作
    async fn search(&self, device_id: &DeviceId) -> BuckyResult<Device> {
        self.search_device(device_id).await
    }

    async fn verfiy_owner(&self, device_id: &DeviceId, device: Option<&Device>) -> BuckyResult<()> {
        self.0.verfiy_owner(device_id, device).await
    }

    async fn verfiy_own_signs(
        &self,
        object_id: &ObjectId,
        object: &Arc<AnyNamedObject>,
    ) -> BuckyResult<()> {
        self.0.verfiy_own_signs(object_id, object).await
    }

    fn clone_cache(&self) -> Box<dyn DeviceCache> {
        Box::new(self.clone())
    }
}
