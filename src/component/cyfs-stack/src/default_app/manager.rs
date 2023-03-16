use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_debug::Mutex;


use std::sync::Arc;

// default app list是device为粒度，还是people？暂时以device为粒度
const DEVICE_DEFAULT_APP_LIST_ID: &str = "device-default-app-list";

struct DefaultAppManager {
    list: Arc<Mutex<Option<DefaultAppList>>>,
    noc: NamedObjectCacheRef,

    // 当前设备id
    device_id: DeviceId,
}

impl DefaultAppManager {
    pub fn new(device_id: &DeviceId, noc: NamedObjectCacheRef) -> Self {
        Self {
            list: Arc::new(Mutex::new(None)),
            noc,
            device_id: device_id.to_owned(),
        }
    }

    pub async fn set_default_app(&self, group: &str, info: DefaultAppInfo) -> BuckyResult<()> {
        // FIXME 这里是否要检查是不是支持的group？考虑向后兼容?
        if !DefaultAppGroupManager::is_known_group(group) {
            let msg = format!("default app group not support yet! group={}", group);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
        }

        // 更新list对象
        // TODO 需要用户确认并签名
        if let Some(list) = self.list.lock().unwrap().as_mut() {
            list.set(group, info);
        } else {
            unreachable!();
        }
        self.save().await;

        Ok(())
    }

    pub fn get_default_app(&self, group: &str) -> Option<DecAppId> {
        self.list
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .get(group)
            .map(|v| v.dec_id.to_owned())
    }

    pub fn get_default_app_by_group_dec_id(&self, group_dec_id: &DecAppId) -> Option<DecAppId> {
        match DEFAULT_APPS.get_default_app_group(group_dec_id.object_id()) {
            Some(group) => self.get_default_app(&group),
            None => None,
        }
    }

    pub async fn init(&self) {
        assert!(self.list.lock().unwrap().is_none());

        let list;
        if let Ok(Some(object)) = self.load_from_noc().await {
            list = object;
        } else {
            list = DefaultAppList::create(
                self.device_id.object_id().to_owned(),
                DEVICE_DEFAULT_APP_LIST_ID,
            );
        };

        *self.list.lock().unwrap() = Some(list);
    }

    async fn load_from_noc(&self) -> BuckyResult<Option<DefaultAppList>> {
        let object_id = DefaultAppList::generate_id(
            self.device_id.object_id().to_owned(),
            DEVICE_DEFAULT_APP_LIST_ID,
        );
        let req = NamedObjectCacheGetObjectRequest {
            protocol: RequestProtocol::Native,
            object_id,
            source: self.device_id.clone(),
            flags: 0,
        };

        if let Some(data) = self.noc.get_object(&req).await? {
            debug!("get default app list object from noc: {}", req.object_id);

            let object_raw = data.object_raw.unwrap();
            let (obj, _) = DefaultAppList::raw_decode(&object_raw).map_err(|e| {
                error!(
                    "decode default app list object from raw data failed! obj={} err={}",
                    object_id, e
                );
                e
            })?;

            Ok(Some(obj))
        } else {
            Ok(None)
        }
    }

    // 保存到noc
    async fn save(&self) {
        let (object_id, object_raw, object) = {
            let guard = self.list.lock().unwrap();
            let list = guard.as_ref().unwrap();
            let object_raw = list.to_vec().unwrap();
            let (object, _) = AnyNamedObject::raw_decode(&object_raw).unwrap();
            let object_id = list.desc().object_id().to_owned();

            (object_id, object_raw, object)
        };
        let info = NamedObjectCacheInsertObjectRequest {
            protocol: RequestProtocol::Native,
            source: self.device_id.to_owned(),
            object_id,
            dec_id: None,
            object_raw,
            object: Arc::new(object),
            flags: 0u32,
        };

        match self.noc.insert_object(&info).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCachePutObjectResult::Accept
                    | NamedObjectCachePutObjectResult::Updated => {
                        info!(
                            "insert default app list object to noc success! id={}",
                            info.object_id
                        );
                    }
                    r @ _ => {
                        // 不应该到这里？因为zone修改后的update_time已经会被更新
                        // FIXME 如果触发了本地时间回滚之类的问题，这里是否需要强制delete然后再插入？
                        error!(
                            "update default app list object to noc but alreay exist! id={}, result={:?}",
                            info.object_id, r
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    "insert default app list object to noc error! id={}, {}",
                    info.object_id, e
                );
            }
        }
    }
}

struct RouterDecHanlder {
    filter: ExpEvaluator,
}

// 以app为粒度进行分发
struct RouterDecHandlers {
    dec_id: DecAppId,

    handlers: Vec<RouterDecHanlder>,
}

struct RouterHanlderManager {}
