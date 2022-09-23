use super::config::AclConfig;
use super::loader::AclFileLoader;
use super::loader::AclLoader;
use super::zone_cache::*;
use crate::resolver::DeviceCache;
use crate::rmeta_api::GlobalStateMetaLocalService;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;
use crate::root_state_api::GlobalStateValidatorManager;

use once_cell::sync::OnceCell;
use std::sync::Arc;

pub(crate) struct AclMatchInstance {
    pub noc: NamedObjectCacheRef,
    pub device_manager: Box<dyn DeviceCache>,
    pub zone_manager: ZoneManagerRef,
}

impl AclMatchInstance {
    pub async fn load_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<NamedObjectCacheObjectData> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object_id: object_id.to_owned(),
            last_access_rpath: None,
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

        let (obj, _) = T::raw_decode(&data.object.object_raw)?;
        Ok(obj)
    }
}

pub(crate) type AclMatchInstanceRef = Arc<AclMatchInstance>;

pub struct AclManager {
    local_global_state_meta: GlobalStateMetaLocalService,
    global_state_validator: GlobalStateValidatorManager,

    zone_manager: ZoneManagerRef,
    file_loader: AclFileLoader,

    local_zone_cache: LocalZoneCache,

    config: OnceCell<AclConfig>,
}

impl AclManager {
    pub(crate) fn new(
        local_global_state_meta: GlobalStateMetaLocalService,
        global_state_validator: GlobalStateValidatorManager,
        noc: NamedObjectCacheRef,
        config_isolate: Option<String>,
        zone_manager: ZoneManagerRef,
    ) -> Self {
        let local_zone_cache = LocalZoneCache::new(zone_manager.clone(), noc.clone());

        let file_loader = AclFileLoader::new(config_isolate.as_ref());

        Self {
            local_global_state_meta,
            global_state_validator,
            zone_manager,
            file_loader,
            local_zone_cache,
            config: OnceCell::new(),
        }
    }

    pub async fn init(&self) -> BuckyResult<()> {
        // First load some acl config
        self.load().await;

        let current_info = self.zone_manager.get_current_info().await?;
        if current_info.zone_role.is_active_ood() {
            // Only init default rmeta on active ood, other ood will been sync to
            self.local_global_state_meta.init().await?;
        }
        
        Ok(())
    }

    async fn load(&self) {
        let mut config = AclConfig::default();
        let mut loader = AclLoader::new(self.file_loader.clone(), &mut config);

        // 加载外部配置
        if let Err(e) = loader.load().await {
            if e.code() == BuckyErrorCode::NotFound {
                warn!("load acl config but not found! {}", e);
            } else {
                warn!("load acl config failed! {}", e);
            }
        }

        self.config.set(config).unwrap();
    }

    pub fn config(&self) -> &AclConfig {
        self.config.get().unwrap()
    }

    pub fn global_state_meta(&self) -> &GlobalStateMetaLocalService {
        &self.local_global_state_meta
    }

    pub fn global_state_validator(&self) -> &GlobalStateValidatorManager {
        &self.global_state_validator
    }

    pub fn get_current_device_id(&self) -> &DeviceId {
        self.zone_manager.get_current_device_id()
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
}

pub(crate) type AclManagerRef = Arc<AclManager>;
