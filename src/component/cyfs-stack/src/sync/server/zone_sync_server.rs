use super::super::global_state::GlobalStateSyncServer;
use super::super::protocol::*;
use super::ping_server::*;
use super::requestor::*;
use super::zone_state::*;
use crate::NamedDataComponents;
use crate::root_state_api::*;
use crate::zone::*;
use cyfs_base::*;
use cyfs_bdt::{DeviceCache, StackGuard};
use cyfs_core::ZoneId;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct ZoneSyncServer {
    ping_server: SyncPingServer,
    zone_state: Arc<ZoneStateManager>,

    noc: NamedObjectCacheRef,

    state_sync_server: GlobalStateSyncServer,

    requestor: Arc<SyncServerRequestorManager>,

    named_data_components: NamedDataComponents,
}

impl ZoneSyncServer {
    pub fn new(
        device_id: &DeviceId,
        zone_id: &ZoneId,
        role_manager: ZoneRoleManager,
        zone_manager: ZoneManagerRef,
        root_state: GlobalStateLocalService,
        noc: NamedObjectCacheRef,
        named_data_components: NamedDataComponents,
        bdt_stack: StackGuard,
        ood_sync_vport: u16,
        device_manager: Box<dyn DeviceCache>,
    ) -> Self {
        let zone_state =
            ZoneStateManager::new(zone_id, root_state.clone(), zone_manager, noc.clone());
        let zone_state = Arc::new(zone_state);

        let ping_server = SyncPingServer::new(zone_state.clone(), role_manager);

        let requestor = SyncServerRequestorManager::new(bdt_stack, device_manager, ood_sync_vport);
        let requestor = Arc::new(requestor);

        let state_sync_server = GlobalStateSyncServer::new(root_state, &device_id, noc.clone());

        Self {
            ping_server,
            zone_state,

            noc,
            state_sync_server,

            requestor,
            named_data_components,
        }
    }

    pub fn zone_state_manager(&self) -> &Arc<ZoneStateManager> {
        &self.zone_state
    }

    pub async fn start(&self) {
        if let Err(_e) = self.zone_state.load().await {
            // FIXME 加载状态失败了，是否继续？
        }

        self.zone_state.start();

        // 开启ping server
        self.ping_server.start();
    }

    // zone的noc插入了新可同步object，seq发生了更新;或者启动时候，从noc获取最新的seq
    // 注意seq是可能发生回滚的，比如机器时间回调等
    pub fn notify_device_zone_state_changed(&self, state: ZoneState, owner_changed: bool) {
        // 异步的通知所有在线设备
        let ping_server = self.ping_server.clone();
        let requestor = self.requestor.clone();

        // let state = state.to_owned();
        async_std::task::spawn(async move {
            let list = ping_server.sync_device_list();
            if !list.is_empty() {
                info!(
                    "will notify online device list: state={}, req={:?}",
                    state, list
                );

                let mut req = SyncZoneRequest {
                    zone_root_state: state.zone_root_state,
                    zone_root_state_revision: state.zone_root_state_revision,
                    zone_role: state.zone_role,
                    ood_work_mode: state.ood_work_mode,
                    owner: None,
                };

                if owner_changed {
                    let object_raw = state.owner.to_vec().unwrap();
                    req.owner = Some(object_raw);
                }

                let device_list = list.into_iter().map(|v| v.device_id).collect();
                let _ = requestor.zone_update(&device_list, req).await;
            } else {
                info!(
                    "online device list is empty!, root={}",
                    state.zone_root_state
                );
            }
        });
    }

    pub async fn device_ping(
        &self,
        source: DeviceId,
        ping_req: SyncPingRequest,
    ) -> BuckyResult<SyncPingResponse> {
        debug!("recv device ping: source={} {:?}", source, ping_req);

        self.zone_state.verify_source(&source).await?;

        self.ping_server.ping(&ping_req).await
    }

    pub async fn sync_diff(
        &self,
        source: DeviceId,
        sync_diff_req: SyncDiffRequest,
    ) -> BuckyResult<SyncDiffResponse> {
        debug!(
            "recv device sync diff: source={} {:?}",
            source, sync_diff_req
        );

        self.zone_state.verify_source(&source).await?;

        self.state_sync_server.sync_diff(&sync_diff_req).await
    }

    pub async fn objects(
        &self,
        source: DeviceId,
        get_req: SyncObjectsRequest,
    ) -> BuckyResult<SyncObjectsResponse> {
        info!("recv device get objects: source={}, {:?}", source, get_req);

        self.zone_state.verify_source(&source).await?;

        let list = self.load_objects(&get_req.list).await;

        // 对所有结果转换为目标类型
        let mut ret_objects: Vec<SelectResponseObjectInfo> = Vec::with_capacity(list.len());
        for item in list.into_iter() {
            let resp_info = SelectResponseObjectInfo {
                meta: SelectResponseObjectMetaInfo {
                    create_dec_id: Some(item.meta.create_dec_id),
                    insert_time: item.meta.insert_time,
                    context: item.meta.context,
                    last_access_rpath: item.meta.last_access_rpath,
                    access_string: Some(item.meta.access_string),
                },
                object: Some(item.object),
            };

            ret_objects.push(resp_info);
        }

        Ok(SyncObjectsResponse {
            objects: ret_objects,
        })
    }

    async fn load_objects(&self, list: &Vec<ObjectId>) -> Vec<NamedObjectCacheObjectData> {
        let mut result = vec![];
        for id in list {
            let req = NamedObjectCacheGetObjectRequest {
                source: RequestSourceInfo::new_local_system(),
                object_id: id.to_owned(),
                last_access_rpath: None,
            };

            match self.noc.get_object(&req).await {
                Ok(Some(data)) => {
                    result.push(data);
                }
                Ok(None) => {
                    warn!("sync get object but not exists! {}", id);
                }
                Err(e) => {
                    error!("sync get object got error! {}, {}", id, e);
                }
            }
        }

        result
    }

    pub async fn chunks(
        &self,
        source: DeviceId,
        get_req: SyncChunksRequest,
    ) -> BuckyResult<SyncChunksResponse> {
        info!("recv device sync chunks: source={}", source);

        self.zone_state.verify_source(&source).await?;

        let req = ExistsChunkRequest {
            chunk_list: get_req.chunk_list,
            states: get_req.states,
        };

        let result = self.named_data_components.ndc.exists_chunks(&req).await?;
        Ok(SyncChunksResponse { result })
    }
}
