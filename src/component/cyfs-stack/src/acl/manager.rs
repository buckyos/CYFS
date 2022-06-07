use super::config::AclConfig;
use super::inner::*;
use super::loader::AclFileLoader;
use super::loader::AclLoader;
use super::relation::AclRelationManager;
use super::request::AclRequest;
use super::request::AclRequestWrapper;
use super::table::{AclItemPosition, AclTableContainer};
use super::{zone_cache::*, AclRequestParams};
use crate::router_handler::RouterHandlersManager;
use crate::resolver::DeviceCache;
use crate::zone::ZoneManager;
use cyfs_base::*;
use cyfs_lib::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;

pub(crate) struct AclMatchInstance {
    pub noc: Box<dyn NamedObjectCache>,
    pub device_manager: Box<dyn DeviceCache>,
    pub zone_manager: ZoneManager,
}

impl AclMatchInstance {
    pub async fn load_object(&self, object_id: &ObjectId) -> BuckyResult<ObjectCacheData> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            protocol: NONProtocol::Native,
            source: self.zone_manager.get_current_device_id().clone(),
            object_id: object_id.to_owned(),
        };

        match self.noc.get_object(&noc_req).await {
            Ok(Some(data)) => {
                info!(
                    "get object from local noc for acl request success: id={}",
                    object_id
                );
                Ok(data)
            }
            Ok(None) => {
                info!(
                    "get object from local noc for acl request but not found: id={}",
                    object_id
                );
                Err(BuckyError::from(BuckyErrorCode::NotFound))
            }
            Err(e) => {
                error!(
                    "get object from local noc for acl request failed: id={}, {}",
                    object_id, e
                );
                Err(e)
            }
        }
    }

    pub async fn load_object_ex<T: for<'de> RawDecode<'de>>(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<T> {
        let data = self.load_object(object_id).await?;

        let (obj, _) = T::raw_decode(&data.object_raw.unwrap())?;
        Ok(obj)
    }
}

pub(crate) type AclMatchInstanceRef = Arc<AclMatchInstance>;

pub struct AclManager {
    match_instance: AclMatchInstanceRef,
    file_loader: AclFileLoader,

    local_zone_cache: LocalZoneCache,

    config: OnceCell<AclConfig>,

    relation_manager: AclRelationManager,
    acl: AclTableContainer,
}

impl AclManager {
    pub(crate) fn new(
        noc: Box<dyn NamedObjectCache>,
        config_isolate: Option<String>,
        device_manager: Box<dyn DeviceCache>,
        zone_manager: ZoneManager,
        router_handlers: RouterHandlersManager,
    ) -> Self {
        let local_zone_cache = LocalZoneCache::new(zone_manager.clone(), noc.clone_noc());

        let match_instance = Arc::new(AclMatchInstance {
            noc,
            device_manager,
            zone_manager,
        });

        let file_loader = AclFileLoader::new(config_isolate.as_ref());
        let relation_manager = AclRelationManager::new(match_instance.clone());
        let acl = AclTableContainer::new(
            file_loader.clone(),
            router_handlers,
            relation_manager.clone(),
        );

        Self {
            match_instance,
            file_loader,
            local_zone_cache,
            config: OnceCell::new(),
            acl,
            relation_manager,
        }
    }

    pub async fn init(&self) {
        // 首先加载配置
        // FIXME 如果加载出错了如何处理？都会fallback到AclTable的默认逻辑
        self.load().await;

        self.relation_manager.start_monitor();
    }

    async fn load(&self) {
        let mut config = AclConfig::default();
        let mut loader = AclLoader::new(
            self.file_loader.clone(),
            &mut config,
            self.acl.clone(),
        );

        // 加载外部配置
        if let Err(e) = loader.load().await {
            if e.code() == BuckyErrorCode::NotFound {
                warn!("load acl config but not found! {}", e);
            } else {
                warn!("load acl config failed! {}", e);
            }
        }

        // 加载内置的默认配置
        AclDefault::load(&self.acl);

        self.config.set(config).unwrap();
    }

    pub fn config(&self) -> &AclConfig {
        self.config.get().unwrap()
    }

    async fn try_match(&self, req: &dyn AclRequest) -> (Option<String>, AclAccess) {
        self.acl.try_match(req).await
    }

    pub(crate) async fn try_match_to_result(&self, req: &dyn AclRequest) -> BuckyResult<()> {
        let (id, access) = self.acl.try_match(req).await;
        // info!("acl match result: {} -> {:?}", req, ret);

        match access {
            AclAccess::Accept => Ok(()),
            AclAccess::Drop => {
                let msg = format!(
                    "req drop by {}'s acl: id={:?}, req={}",
                    self.get_current_device_id(),
                    id,
                    req
                );
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::Ignored, msg))
            }
            AclAccess::Reject => {
                let msg = format!(
                    "req reject by {}'s acl: id={:?}, req={}",
                    self.get_current_device_id(),
                    id,
                    req
                );
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
            }
            AclAccess::Pass => Ok(()),
        }
    }

    pub(crate) fn new_acl_request(&self, param: AclRequestParams) -> AclRequestWrapper {
        AclRequestWrapper::new_from_params(self.match_instance.clone(), param)
    }

    pub fn get_current_device_id(&self) -> &DeviceId {
        self.match_instance.zone_manager.get_current_device_id()
    }

    pub async fn is_current_zone_device(&self, device_id: &DeviceId) -> BuckyResult<bool> {
        self.local_zone_cache
            .is_current_zone_device(device_id)
            .await
    }

    // 同协议栈检查
    pub async fn check_local_permit(&self, service: &str, device: &DeviceId) -> BuckyResult<()> {
        if self.get_current_device_id() != device {
            let msg = format!(
                "{} service valid only in current device! source/target={}",
                service, device
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        Ok(())
    }

    // 同zone权限检查，一些服务只允许同zone内设备访问
    pub async fn check_local_zone_permit(
        &self,
        service: &str,
        device: &DeviceId,
    ) -> BuckyResult<()> {
        let ac = self.is_current_zone_device(device).await?;
        if !ac {
            let msg = format!(
                "{} service valid only in current zone! source/target={}",
                service, device
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        } else {
            Ok(())
        }
    }

    // 动态添加和移除acl条目
    pub fn add_item(&self, pos: AclItemPosition, value: &str) -> BuckyResult<()> {
        self.acl.add_item(pos, value)
    }

    pub fn remove_item(&self, id: &str) -> BuckyResult<()> {
        self.acl.remove_item(id)
    }
}

pub(crate) type AclManagerRef = Arc<AclManager>;
