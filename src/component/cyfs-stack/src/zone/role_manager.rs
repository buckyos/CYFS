use super::zone_manager::*;
use crate::acl::AclManagerRef;
use crate::config::StackGlobalConfig;
use crate::events::RouterEventsManager;
use crate::interface::{SyncListenerManager, SyncListenerManagerParams};
use crate::meta::MetaCacheRef;
use crate::root_state_api::GlobalStateLocalService;
use crate::{sync::*, NamedDataComponents};
use crate::util_api::UtilService;
use cyfs_base::*;
use cyfs_bdt::{DeviceCache, StackGuard};
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;

const ROLE_MANAGER_HANDLER_ID: &str = "system_role_manager_controller";

struct OnPeopleUpdateWatcher {
    owner: ZoneRoleManager,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for OnPeopleUpdateWatcher
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        info!(
            "recv people update request: {}",
            param.request.object.object_id
        );

        let result = match self.owner.on_update_owner(&param.request.object).await {
            Ok(()) => Ok(NONPostObjectInputResponse { object: None }),
            Err(e) => Err(e),
        };

        let resp = RouterHandlerPostObjectResult {
            action: RouterHandlerAction::Response,
            request: None,
            response: Some(result),
        };

        Ok(resp)
    }
}

#[derive(Clone)]
pub struct ZoneRoleManager {
    device_id: DeviceId,
    zone_manager: ZoneManagerRef,
    noc: NamedObjectCacheRef,
    raw_meta_cache: MetaCacheRef,
    acl_manager: AclManagerRef,

    config: StackGlobalConfig,

    // sync服务相关
    sync_server: Arc<OnceCell<Arc<ZoneSyncServer>>>,
    sync_client: Arc<OnceCell<Arc<DeviceSyncClient>>>,
    sync_interface: Arc<OnceCell<SyncListenerManager>>,

    // events
    event_manager: RouterEventsManager,
}

impl ZoneRoleManager {
    pub(crate) fn new(
        device_id: DeviceId,
        zone_manager: ZoneManagerRef,
        noc: NamedObjectCacheRef,
        raw_meta_cache: MetaCacheRef,
        acl_manager: AclManagerRef,
        event_manager: RouterEventsManager,
        config: StackGlobalConfig,
    ) -> Self {
        Self {
            device_id,
            zone_manager,
            noc,
            raw_meta_cache,
            acl_manager,
            event_manager,

            config,

            sync_server: Arc::new(OnceCell::new()),
            sync_client: Arc::new(OnceCell::new()),
            sync_interface: Arc::new(OnceCell::new()),
        }
    }

    pub fn zone_manager(&self) -> &ZoneManagerRef {
        &self.zone_manager
    }

    pub(crate) fn sync_server(&self) -> Option<&Arc<ZoneSyncServer>> {
        self.sync_server.get()
    }
    pub(crate) fn sync_client(&self) -> Option<&Arc<DeviceSyncClient>> {
        self.sync_client.get()
    }

    pub(crate) async fn init_root_state_access_mode(&self) -> BuckyResult<()> {
        let current_zone_info = self.zone_manager.get_current_info().await?;
        let access_mode = match current_zone_info.zone_role {
            ZoneRole::ActiveOOD => GlobalStateAccessMode::Write,
            ZoneRole::StandbyOOD => GlobalStateAccessMode::Read,
            ZoneRole::ReservedOOD => GlobalStateAccessMode::Read,
            ZoneRole::Device => GlobalStateAccessMode::Write,
        };

        self.config
            .change_access_mode(GlobalStateCategory::RootState, access_mode);

        Ok(())
    }

    pub async fn on_update_owner(&self, object: &NONObjectInfo) -> BuckyResult<()> {
        let mut object = object.clone();
        if object.object.is_none() {
            object.decode()?;
        }

        // verify owner's id if match
        let current_info = self.zone_manager.get_current_info().await?;
        if current_info.owner_id != object.object_id {
            let msg = format!(
                "unmatch zone's owner_id: current={}, got={}",
                current_info.owner_id, object.object_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        // check update_time if is newer
        let current_update_time = current_info.owner.get_update_time();
        let new_update_time = object.object.as_ref().unwrap().get_update_time();
        if current_update_time >= new_update_time {
            let msg = format!(
                "zone's owner's update_time is same or older: current={}, got={}",
                current_update_time, new_update_time
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
        }

        // should verify the owner's body sign
        let obj = object.object.as_ref().unwrap();
        self.zone_manager
            .device_manager()
            .verfiy_own_signs(&object.object_id, &obj)
            .await
            .map_err(|e| {
                error!("post owner but verify sign failed! {}", e);
                e
            })?;

        // try update to noc
        let updated = self.update_owner_to_noc(object).await?;
        if !updated {
            return Ok(());
        }

        let this = self.clone();
        async_std::task::spawn(async move {
            let _ = this.notify_owner_changed().await;
        });

        Ok(())
    }

    async fn update_owner_to_noc(&self, object: NONObjectInfo) -> BuckyResult<bool> {
        let owner_id = object.object_id.clone();

        // save to noc...
        let req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: Some(AccessString::full_except_write().value()),
        };

        match self.noc.put_object(&req).await {
            Ok(resp) => {
                let updated = match resp.result {
                    NamedObjectCachePutObjectResult::Accept
                    | NamedObjectCachePutObjectResult::Updated => {
                        info!("device update owner object to noc success: {}", owner_id);
                        true
                    }
                    NamedObjectCachePutObjectResult::AlreadyExists => {
                        warn!(
                            "device update owner object but already exists: {}",
                            owner_id
                        );
                        false
                    }
                    NamedObjectCachePutObjectResult::Merged => {
                        warn!(
                            "device update owner object but signs merged success: {}",
                            owner_id
                        );
                        true
                    }
                };

                Ok(updated)
            }
            Err(e) => {
                error!(
                    "device update owner object to noc failed: {} {}",
                    owner_id, e
                );
                Err(e)
            }
        }
    }

    // notify zone's owner maybe changed
    pub(crate) async fn notify_owner_changed(&self) -> BuckyResult<()> {
        let current_info = self.zone_manager.get_current_info().await?;

        let (zone, changed) = self.flush_owner().await?;
        if !changed {
            info!("zone notify flush owner but not changed!");
            return Ok(());
        }

        let zone_id = zone.zone_id();
        let owner_id = zone.owner();

        info!(
            "current zone's owner changed! now will flush current zone info! zone={}, owner={}",
            zone_id, owner_id
        );
        self.zone_manager.remove_zone(&zone_id).await;

        // gen new current zone info
        let new_info = self.zone_manager.get_current_info().await?;
        info!(
            "zone info changed: current={}, latest={}",
            current_info, new_info
        );

        self.on_zone_changed(current_info, new_info).await;

        Ok(())
    }

    async fn emit_zone_role_changed_event(
        &self,
        param: ZoneRoleChangedEventRequest,
    ) -> BuckyResult<()> {
        let event = self.event_manager.events().try_zone_role_changed_event();
        if event.is_none() {
            return Ok(());
        }

        let mut emitter = event.unwrap().emitter();
        let resp = emitter.emit(param).await;
        info!("zone role changed event resp: {}", resp);

        Ok(())
    }

    async fn on_zone_changed(
        &self,
        current_info: Arc<CurrentZoneInfo>,
        new_info: Arc<CurrentZoneInfo>,
    ) {
        // if zone_role changed, cyfs-stack should be restart to apply the change!
        if current_info.zone_role != new_info.zone_role {
            warn!(
                "zone role changed: {} -> {}",
                current_info.zone_role, new_info.zone_role
            );

            let param = ZoneRoleChangedEventRequest {
                current_role: current_info.zone_role,
                new_role: new_info.zone_role,
            };

            if let Err(e) = self.emit_zone_role_changed_event(param).await {
                error!("emit zone role changed event error! {}", e);
            } else {
                info!("emit zone role changed event success!");
            }
        } else {
            match new_info.zone_role {
                ZoneRole::Device => {
                    if current_info.zone_device_ood_id != new_info.zone_device_ood_id {
                        info!(
                            "zone ood device id changed! now will notify sync client {} -> {}",
                            current_info.zone_device_ood_id, new_info.zone_device_ood_id
                        );

                        match self.sync_client.get() {
                            Some(client) => {
                                let _ = client.notify_zone_ood_chanegd().await;
                            }
                            None => {
                                warn!("sync client not init yet!");
                            }
                        }
                    }
                }
                ZoneRole::ActiveOOD | ZoneRole::ReservedOOD | ZoneRole::StandbyOOD => {
                    match self.sync_server.get() {
                        Some(server) => {
                            let zone_state = server.zone_state_manager().get_zone_state().await;
                            server.notify_device_zone_state_changed(zone_state, true);
                        }
                        None => {
                            warn!("sync server not init yet!");
                        }
                    };
                }
            }
        }
    }

    async fn flush_owner(&self) -> BuckyResult<(Zone, bool)> {
        let zone = self.zone_manager.get_current_zone().await?;
        let owner_id = zone.owner();

        // first load owner from meta, but meta maybe not valid in solo mode,
        // then we should load from noc(new owner object should had beed put to noc)
        let mut new_owner = match self.load_owner_from_meta(owner_id).await {
            Ok(Some(owner)) => Some(owner),
            Ok(None) => {
                warn!("flush owner from meta chain but not found! id={}", owner_id);

                None
            }
            Err(e) => {
                warn!(
                    "flush owner from meta chain but failed! id={}, {}",
                    owner_id, e
                );

                None
            }
        };

        if new_owner.is_none() {
            // try load new owner object from local noc
            new_owner = match self.load_owner_from_noc(&owner_id).await {
                Ok(v) => v,
                Err(_) => {
                    warn!("load owner from local noc but got error! id={}", owner_id);
                    None
                }
            };
        };

        if new_owner.is_none() {
            let msg = format!(
                "flush owner from meta chain and local noc not found or got error! id={}",
                owner_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let new_owner = new_owner.unwrap();

        let current_info = self.zone_manager.get_current_info().await?;
        let current_owner = current_info.owner.clone();

        // compare
        match Self::check_zone_changed(&current_owner, &new_owner) {
            Ok(ret) => Ok((zone, ret)),
            Err(e) => {
                error!("check zone change but got error! {}", e);
                Err(e)
            }
        }
    }

    fn check_zone_changed(
        current_owner: &Arc<AnyNamedObject>,
        latest_owner: &Arc<AnyNamedObject>,
    ) -> BuckyResult<bool> {
        if current_owner.ood_work_mode()? != latest_owner.ood_work_mode()? {
            info!(
                "people ood_work_mode changed! {:?} -> {:?}",
                current_owner.ood_work_mode(),
                latest_owner.ood_work_mode()
            );
            return Ok(true);
        }

        if current_owner.ood_list()? != latest_owner.ood_list()? {
            info!(
                "people ood_list changed! {:?} -> {:?}",
                current_owner.ood_list(),
                latest_owner.ood_list()
            );
            return Ok(true);
        }

        Ok(false)
    }

    async fn load_owner_from_meta(
        &self,
        people_id: &ObjectId,
    ) -> BuckyResult<Option<Arc<AnyNamedObject>>> {
        let resp = self
            .raw_meta_cache
            .get_object(&people_id)
            .await
            .map_err(|e| {
                error!(
                    "flush people from meta chain error! id={}, {}",
                    people_id, e
                );
                e
            })?;

        if resp.is_none() {
            return Ok(None);
        }

        let data = resp.unwrap();
        Ok(Some(data.object))
    }

    async fn load_owner_from_noc(
        &self,
        people_id: &ObjectId,
    ) -> BuckyResult<Option<Arc<AnyNamedObject>>> {
        let req = NamedObjectCacheGetObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object_id: people_id.to_owned(),
            last_access_rpath: None,
            flags: 0,
        };

        if let Ok(Some(data)) = self.noc.get_object(&req).await {
            Ok(data.object.object)
        } else {
            error!("load people from noc but not found! id={}", people_id);
            Ok(None)
        }
    }

    pub(crate) async fn init(
        &self,
        root_state: &GlobalStateLocalService,
        bdt_stack: &StackGuard,
        device_manager: &Box<dyn DeviceCache>,
        router_handlers: &RouterHandlerManagerProcessorRef,
        util_service: &Arc<UtilService>,
        named_data_components: NamedDataComponents,
    ) -> BuckyResult<()> {
        let current_zone_info = self.zone_manager.get_current_info().await?;
        match current_zone_info.zone_role {
            ZoneRole::ActiveOOD => {
                self.start_sync_server(root_state, bdt_stack, device_manager, named_data_components)
                    .await?;
            }
            ZoneRole::StandbyOOD => {
                self.start_sync_server(root_state, bdt_stack, device_manager, named_data_components.clone())
                    .await?;

                self.start_sync_client(
                    root_state,
                    bdt_stack,
                    device_manager,
                    util_service,
                    named_data_components,
                    true,
                    true,
                )
                .await?;
            }
            ZoneRole::ReservedOOD => {
                self.start_sync_server(root_state, bdt_stack, device_manager, named_data_components.clone())
                    .await?;

                self.start_sync_client(
                    root_state,
                    bdt_stack,
                    device_manager,
                    util_service,
                    named_data_components,
                    true,
                    false,
                )
                .await?;
            }
            ZoneRole::Device => {
                self.start_sync_client(
                    root_state,
                    bdt_stack,
                    device_manager,
                    util_service,
                    named_data_components,
                    true,
                    false,
                )
                .await?;
            }
        }

        self.register_router_handler(router_handlers).await?;

        self.start_sync_interface(bdt_stack).await?;

        Ok(())
    }

    async fn register_router_handler(
        &self,
        router_handlers: &RouterHandlerManagerProcessorRef,
    ) -> BuckyResult<()> {
        // let zone = self.zone_manager.get_current_zone().await?;
        // let owner = zone.owner();
        // let filter = format!("object_id == {}", owner.to_string());

        // add post_object handler for app_manager's action cmd
        let routine = OnPeopleUpdateWatcher {
            owner: self.clone(),
        };

        let req_path = RequestGlobalStatePath::new_system_dec(Some(CYFS_SYSTEM_ROLE_VIRTUAL_PATH));
        if let Err(e) = router_handlers
            .post_object()
            .add_handler(
                RouterHandlerChain::Handler,
                ROLE_MANAGER_HANDLER_ID,
                1,
                None,
                Some(req_path.to_string()),
                RouterHandlerAction::Default,
                Some(Box::new(routine)),
            )
            .await
        {
            error!("add role_manager controller handler error! {}", e);
            return Err(e);
        }

        Ok(())
    }

    async fn start_sync_server(
        &self,
        root_state: &GlobalStateLocalService,
        bdt_stack: &StackGuard,
        device_manager: &Box<dyn DeviceCache>,
        named_data_components: NamedDataComponents,
    ) -> BuckyResult<()> {
        let current_zone_info = self.zone_manager.get_current_info().await?;

        info!(
            "will start sync server: zone={}, ood={}, role={}",
            current_zone_info.zone_id,
            current_zone_info.zone_device_ood_id,
            current_zone_info.zone_role,
        );

        let server = ZoneSyncServer::new(
            &self.device_id,
            &current_zone_info.zone_id,
            self.clone(),
            self.zone_manager.clone(),
            root_state.clone(),
            self.noc.clone(),
            named_data_components,
            bdt_stack.clone(),
            cyfs_base::NON_STACK_SYNC_BDT_VPORT,
            device_manager.clone_cache(),
        );

        server.start().await;

        if let Err(_) = self.sync_server.set(Arc::new(server)) {
            unreachable!();
        }

        Ok(())
    }

    async fn start_sync_client(
        &self,
        root_state: &GlobalStateLocalService,
        bdt_stack: &StackGuard,
        device_manager: &Box<dyn DeviceCache>,
        util_service: &Arc<UtilService>,
        named_data_components: NamedDataComponents,
        enable_ping: bool,
        enable_sync: bool,
    ) -> BuckyResult<()> {
        let current_zone_info = self.zone_manager.get_current_info().await?;

        info!(
            "will start sync client: current_device={}, zone={}, ood={}, role={}, enable_sync={}",
            current_zone_info.device_id,
            current_zone_info.zone_id,
            current_zone_info.zone_device_ood_id,
            current_zone_info.zone_role,
            enable_sync,
        );

        let client = DeviceSyncClient::new(
            self.clone(),
            &self.zone_manager,
            root_state.clone(),
            bdt_stack,
            cyfs_base::NON_STACK_SYNC_BDT_VPORT,
            device_manager.clone_cache(),
            self.noc.clone(),
            self.acl_manager.clone(),
            named_data_components,
        )
        .await?;

        client.start().await;

        client.enable_sync(enable_sync);

        if enable_ping {
            client.start_ping();
        }

        let sync_client = Arc::new(client);
        util_service
            .local_service()
            .bind_sync_client(sync_client.clone());

        if let Err(_) = self.sync_client.set(sync_client) {
            unreachable!();
        }

        Ok(())
    }

    async fn start_sync_interface(&self, bdt_stack: &StackGuard) -> BuckyResult<()> {
        let params = SyncListenerManagerParams {
            bdt_stack: bdt_stack.to_owned(),
            bdt_listeners: vec![cyfs_base::NON_STACK_SYNC_BDT_VPORT],
        };

        let mut interface = SyncListenerManager::new();
        interface.init(params, self.sync_server.get(), self.sync_client.get());
        interface.start().await?;

        if let Err(_) = self.sync_interface.set(interface) {
            unreachable!();
        }

        Ok(())
    }
}
