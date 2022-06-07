use super::super::protocol::*;
use super::device_state::*;
use super::object_sync_client::*;
use super::ping_client::SyncPingClient;
use super::requestor::SyncClientRequestor;
use crate::acl::AclManagerRef;
use crate::root_state_api::GlobalStateLocalService;
use crate::zone::ZoneRoleManager;
use crate::zone::*;
use cyfs_chunk_cache::ChunkManager;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, DeviceId};
use cyfs_lib::*;
use cyfs_bdt::{DeviceCache, StackGuard};

use once_cell::sync::OnceCell;
use std::sync::{Arc, RwLock};

// 追踪ping的状态更新，并发起主动同步
struct SyncStateInductorImpl {
    role_manager: ZoneRoleManager,
    sync_client: OnceCell<Arc<ObjectSyncClient>>,
}

impl SyncStateInductorImpl {
    pub fn new(role_manager: ZoneRoleManager) -> Self {
        Self {
            role_manager,
            sync_client: OnceCell::new(),
        }
    }
}

#[derive(Clone)]
struct SyncStateInductor(Arc<SyncStateInductorImpl>);

impl SyncStateInductor {
    pub fn new(role_manager: ZoneRoleManager) -> Self {
        Self(Arc::new(SyncStateInductorImpl::new(role_manager)))
    }

    pub fn init(&self, sync_client: Arc<ObjectSyncClient>) {
        if let Err(_) = self.0.sync_client.set(sync_client) {
            unreachable!("sync_client should not set for the second time!");
        }
    }

    pub fn sync_client(&self) -> &Arc<ObjectSyncClient> {
        self.0.sync_client.get().unwrap()
    }
}

impl DeviceStateManagerEvent for SyncStateInductor {
    fn zone_state_update(&self, old_zone_state: LocalZoneState, new_zone_state: LocalZoneState) {
        let sync_client = self.sync_client().clone();
        async_std::task::spawn(async move {
            sync_client.sync().await;
        });

        if old_zone_state.zone_role != new_zone_state.zone_role
            || old_zone_state.ood_work_mode != new_zone_state.ood_work_mode
        {
            warn!("ood's zone_role or ood_work_mode changed! now will notify role manager");
            let imp = self.0.clone();
            async_std::task::spawn(async move {
                let _ = imp.role_manager.notify_owner_changed().await;
            });
        }
    }
}

pub(crate) struct DeviceSyncClient {
    device_id: DeviceId,
    ood_device_id: RwLock<DeviceId>,

    zone_manager: ZoneManager,
    state_manager: Arc<DeviceStateManager>,
    acl_manager: AclManagerRef,

    ping_client: SyncPingClient,
    sync_client: Arc<ObjectSyncClient>,

    bdt_stack: StackGuard,
    device_manager: Box<dyn DeviceCache>,
    ood_sync_vport: u16,
    requestor: Arc<SyncClientRequestor>,
}

impl DeviceSyncClient {
    pub async fn new(
        role_manager: ZoneRoleManager,
        zone_manager: &ZoneManager,
        root_state: GlobalStateLocalService,
        bdt_stack: &StackGuard,
        ood_sync_vport: u16,
        device_manager: Box<dyn DeviceCache>,
        raw_noc: Box<dyn NamedObjectCache>,
        acl_manager: AclManagerRef,
        chunk_manager: Arc<ChunkManager>,
    ) -> BuckyResult<Self> {
        let zone_info = zone_manager.get_current_info().await?;
        let device_id = zone_info.device_id.clone();
        let ood_device_id = zone_info.zone_device_ood_id.clone();
        assert!(device_id != ood_device_id);

        let requestor =
            Self::init_requestor(bdt_stack, &ood_device_id, ood_sync_vport, &device_manager)
                .await?;
        let requestor = Arc::new(requestor);

        let event = SyncStateInductor::new(role_manager.clone());
        let state_manager = DeviceStateManager::new(
            &device_id,
            raw_noc.clone_noc(),
            root_state.clone(),
            zone_manager.clone(),
            Box::new(event.clone()),
        );
        let state_manager = Arc::new(state_manager);
        let _ = state_manager.load().await;

        let sync_client = ObjectSyncClient::new(
            &ood_device_id,
            root_state,
            state_manager.clone(),
            requestor.clone(),
            raw_noc,
            bdt_stack.clone(),
            chunk_manager,
        );
        let sync_client = Arc::new(sync_client);

        // 真正初始化state_manager的事件
        event.init(sync_client.clone());

        let ping_client = SyncPingClient::new(
            &device_id,
            state_manager.clone(),
            requestor.clone(),
            sync_client.clone(),
            zone_info.zone_role.clone(),
            role_manager,
        )
        .await?;

        let ret = Self {
            device_id,
            ood_device_id: RwLock::new(ood_device_id),

            zone_manager: zone_manager.clone(),
            state_manager,
            acl_manager,

            ping_client,
            sync_client,

            bdt_stack: bdt_stack.clone(),
            device_manager,
            ood_sync_vport,
            requestor,
        };

        Ok(ret)
    }

    fn ood_device_id(&self) -> DeviceId {
        self.ood_device_id.read().unwrap().clone()
    }

    pub fn enable_sync(&self, enable: bool) -> bool {
        let last = self.sync_client.enable_sync(enable);
        if enable {
            // 立即开始一次同步
            let sync_client = self.sync_client.clone();
            async_std::task::spawn(async move {
                sync_client.sync().await;
            });
        }

        last
    }

    async fn init_requestor(
        bdt_stack: &StackGuard,
        ood_device_id: &DeviceId,
        ood_sync_vport: u16,
        device_manager: &Box<dyn DeviceCache>,
    ) -> BuckyResult<SyncClientRequestor> {
        let device = device_manager.search(ood_device_id).await?;

        assert!(ood_sync_vport > 0);
        let bdt_requestor = BdtHttpRequestor::new(bdt_stack.clone(), device, ood_sync_vport);
        let requestor = SyncClientRequestor::new(Box::new(bdt_requestor));

        info!(
            "init sync client bdt requestor to ood={} success!",
            ood_device_id
        );

        Ok(requestor)
    }

    pub async fn start(&self) {
        if let Err(_e) = self.state_manager.load().await {
            // FIXME 加载状态失败了，是否继续？
        }

        self.state_manager.start();
    }

    pub fn start_ping(&self) {
        // 开启ping
        self.ping_client.start(DeviceSyncState::OnlineAccept);
    }

    pub async fn stop_ping(&self) {
        self.ping_client.stop().await
    }

    pub fn wakeup_ping(&self) {
        self.ping_client.wakeup_ping();
    }

    pub fn verify_source(&self, source: &DeviceId) -> BuckyResult<()> {
        let ood_device_id = self.ood_device_id();
        if ood_device_id != *source {
            let msg = format!(
                "ood device not match: source device={}, ood={}",
                source, ood_device_id
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotMatch, msg));
        }

        Ok(())
    }

    // ood发来的通知
    pub async fn zone_update(
        &self,
        source: DeviceId,
        zone_req: SyncZoneRequest,
    ) -> BuckyResult<()> {
        // 校验source
        self.verify_source(&source)?;

        info!("recv zone update notify: {:?}", zone_req);

        let zone_state = LocalZoneState {
            zone_root_state: Some(zone_req.zone_root_state),
            zone_root_state_revision: zone_req.zone_root_state_revision,
            zone_role: zone_req.zone_role,
            ood_work_mode: zone_req.ood_work_mode,
        };

        if let Some(object_raw) = zone_req.owner {
            let _ = self.state_manager.update_owner(object_raw).await;
        }

        self.state_manager.update_zone_state(zone_state);

        Ok(())
    }

    pub async fn get_sync_state(
        &self,
        source: DeviceId,
        flush: bool,
    ) -> BuckyResult<DeviceSyncStatus> {
        // check permit, must in the same zone
        self.acl_manager
            .check_local_zone_permit("sync in get_sync_state", &source)
            .await?;

        let ping_ret = self.ping_client.ping_state(flush).await;
        let device_state = self.state_manager.get_device_state().await?;
        let zone_state = self.state_manager.get_zone_state();

        let status = DeviceSyncStatus {
            ood_device_id: self.ood_device_id(),
            enable_sync: self.sync_client.is_enable_sync(),

            device_root_state: device_state.root_state,
            device_root_state_revision: device_state.root_state_revision,

            zone_root_state: zone_state.zone_root_state,
            zone_root_state_revision: zone_state.zone_root_state_revision,

            last_ping_result: ping_ret.last_ping_result,
            last_ping_time: ping_ret.last_ping_time,
            last_success_ping_time: ping_ret.last_success_ping_time,
            retry_count: ping_ret.retry_count,
        };

        Ok(status)
    }

    // 获取目标ood的状态，目前都是通过ping推导出来的
    pub async fn get_ood_status(&self) -> BuckyResult<OODStatus> {
        let device_state = self.state_manager.get_device_state().await?;
        let zone_state = self.state_manager.get_zone_state();

        let mut status = OODStatus {
            network: OODNetworkType::Unknown,

            first_ping: 0,
            first_success_ping: 0,
            last_success_ping: 0,

            last_ping: 0,
            last_ping_result: 0,

            ping_count: 0,
            ping_success_count: 0,

            cont_fail_count: 0,

            ping_avg_during: 0,
            ping_max_during: 0,
            ping_min_during: 0,

            ood_device_id: self.ood_device_id(),
            enable_sync: self.sync_client.is_enable_sync(),

            device_root_state: device_state.root_state,
            device_root_state_revision: device_state.root_state_revision,

            zone_root_state: zone_state.zone_root_state,
            zone_root_state_revision: zone_state.zone_root_state_revision,
        };

        self.ping_client.fill_ood_status(&mut status);

        Ok(status)
    }

    pub(crate) async fn notify_zone_ood_chanegd(&self) -> BuckyResult<()> {
        let zone_info = self.zone_manager.get_current_info().await?;
        let current_ood_device_id = self.ood_device_id();
        let ood_device_id = zone_info.zone_device_ood_id.clone();
        if current_ood_device_id == ood_device_id {
            return Ok(());
        }

        info!(
            "zone ood device id changed: {} -> {}",
            current_ood_device_id, ood_device_id
        );

        let device = self
            .device_manager
            .search(&ood_device_id)
            .await
            .map_err(|e| {
                error!("get new ood device failed! id={}, {}", ood_device_id, e);
                e
            })?;

        let bdt_requestor =
            BdtHttpRequestor::new(self.bdt_stack.clone(), device, self.ood_sync_vport);
        self.requestor.reset_requestor(Box::new(bdt_requestor));

        {
            *self.ood_device_id.write().unwrap() = ood_device_id.clone();
        }

        self.sync_client.wakeup_sync();

        Ok(())
    }
}
