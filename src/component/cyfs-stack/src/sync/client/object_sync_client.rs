use super::super::global_state::*;
use super::super::protocol::*;
use super::device_state::*;
use super::requestor::SyncClientRequestor;
use crate::root_state_api::{GlobalStateLocalService, RootInfo};
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_chunk_cache::ChunkManager;
use cyfs_debug::Mutex;
use cyfs_lib::*;

use futures::future::{AbortHandle, Abortable};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// sync的重试间隔
const SYNC_RETRY_MIN_INTERVAL_SECS: u64 = 10;
const SYNC_RETRY_MAX_INTERVAL_SECS: u64 = 60 * 5;

pub(super) struct ObjectSyncClient {
    state_manager: Arc<DeviceStateManager>,

    requestor: Arc<SyncClientRequestor>,

    state_sync_helper: GlobalStateSyncHelper,

    during: AtomicBool,
    enable: AtomicBool,

    sync_waker: Mutex<Option<AbortHandle>>,

    state_cache: SyncObjectsStateCache,

    bdt_stack: StackGuard,
    chunk_manager: Arc<ChunkManager>,
}

impl ObjectSyncClient {
    pub fn new(
        device_id: &DeviceId,
        root_state: GlobalStateLocalService,
        state_manager: Arc<DeviceStateManager>,
        requestor: Arc<SyncClientRequestor>,
        noc: NamedObjectCacheRef,
        bdt_stack: StackGuard,
        chunk_manager: Arc<ChunkManager>,
    ) -> Self {
        let state_sync_helper = GlobalStateSyncHelper::new(root_state, device_id, noc);

        // TODO 目前state_cache只在一次协议栈进程周期有效，不做持久化缓存
        let state_cache = SyncObjectsStateCache::new();

        Self {
            state_sync_helper,
            state_manager,
            requestor,
            state_cache,
            bdt_stack,
            chunk_manager,

            during: AtomicBool::new(false),
            enable: AtomicBool::new(false),
            sync_waker: Mutex::new(None),
        }
    }

    pub fn enable_sync(&self, enable: bool) -> bool {
        let ret = self.enable.swap(enable, Ordering::SeqCst);
        if ret != enable {
            info!("object sync client enable changed! {} -> {}", ret, enable);
        }

        ret
    }

    pub fn is_enable_sync(&self) -> bool {
        self.enable.load(Ordering::SeqCst)
    }

    fn is_during_sync(&self) -> bool {
        self.during.load(Ordering::SeqCst)
    }

    fn enter_during_sync(&self) -> bool {
        match self
            .during
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(v) => v,
            Err(v) => v,
        }
    }

    fn leave_during_sync(&self) {
        self.during.store(false, Ordering::SeqCst);
    }

    pub fn wakeup_sync(&self) {
        let waker = self.sync_waker.lock().unwrap().take();
        if let Some(waker) = waker {
            info!("now will wakeup object sync!");
            waker.abort();
        }
    }

    // 同步，直到状态一致，失败后会自动重试
    pub async fn sync(&self) {
        if !self.is_enable_sync() {
            return;
        }

        if self.enter_during_sync() {
            info!("already during sync");
            return;
        }

        // 重试间隔
        let mut retry_interval = SYNC_RETRY_MIN_INTERVAL_SECS;

        let device_state = loop {
            match self.sync_impl().await {
                Ok(device_state) => break device_state,
                Err(e) => {
                    error!(
                        "sync error, now will retry after {} secs: {}",
                        retry_interval, e
                    );

                    // 等待重试，并允许被提前唤醒
                    let (abort_handle, abort_registration) = AbortHandle::new_pair();
                    let fut = Abortable::new(
                        async_std::task::sleep(std::time::Duration::from_secs(retry_interval)),
                        abort_registration,
                    );

                    {
                        *self.sync_waker.lock().unwrap() = Some(abort_handle);
                    }

                    match fut.await {
                        Ok(_) => {
                            info!("sync retry timeout");
                            let _ = self.sync_waker.lock().unwrap().take();
                        }
                        Err(futures::future::Aborted { .. }) => {
                            info!("sync retry wakeup");
                        }
                    };

                    retry_interval *= 2;
                    if retry_interval >= SYNC_RETRY_MAX_INTERVAL_SECS {
                        retry_interval = SYNC_RETRY_MAX_INTERVAL_SECS;
                    }
                }
            }
        };

        info!("sync complete, current device state: {}", device_state);

        assert!(self.is_during_sync());
        self.leave_during_sync();
    }

    // 同步，直到seq和zone_seq一致，或者出错后退出
    async fn sync_impl(&self) -> BuckyResult<DeviceState> {
        loop {
            let device_state = self.state_manager.get_device_state().await?;
            let zone_state = self.state_manager.get_zone_state();

            if zone_state.zone_root_state.is_none() {
                break Ok(device_state);
            }

            // don't sync if current device still use the deactive ood
            if zone_state.zone_role != ZoneRole::ActiveOOD {
                warn!(
                    "current zone's ood is not active ood! role={}",
                    zone_state.zone_role
                );
                break Ok(device_state);
            }

            if device_state.root_state == *zone_state.zone_root_state.as_ref().unwrap() {
                trace!(
                    "device state match zone state! device={}, zone={}",
                    device_state,
                    zone_state
                );
                break Ok(device_state);
            }

            match self.sync_once(&device_state, &zone_state).await {
                Ok(()) => {
                    break Ok(device_state);
                }
                Err(e) => break Err(e),
            }
        }
    }

    async fn sync_once(
        &self,
        device_state: &DeviceState,
        zone_state: &LocalZoneState,
    ) -> BuckyResult<()> {
        info!(
            "will sync root_state: {} -> {}",
            device_state.root_state,
            zone_state.zone_root_state.as_ref().unwrap()
        );

        let req = SyncDiffRequest {
            category: GlobalStateCategory::RootState,
            path: "/".to_owned(),
            dec_id: None,
            current: None,
        };

        let client = GlobalStateSyncClient::new(
            self.requestor.clone(),
            self.state_sync_helper.clone(),
            self.state_cache.clone(),
            self.bdt_stack.clone(),
            self.chunk_manager.clone(),
        );
        let (had_saved_error, result) = client.sync(req).await?;

        if had_saved_error {
            warn!("sync root_state but had saved errors! result={:?}", result);
        }

        match result.target {
            Some(target) => {
                let root = RootInfo {
                    root_state: Some(target),
                    revision: result.revision,
                };

                self.state_sync_helper
                    .global_state()
                    .state()
                    .direct_set_root_state(root, None)
                    .await
            }
            None => {
                warn!("sync root_state but target is empty!");

                Ok(())
            }
        }
    }
}
