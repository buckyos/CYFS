use super::super::protocol::*;
use super::device_state::*;
use super::object_sync_client::ObjectSyncClient;
use super::ping_status::*;
use super::requestor::*;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;
use crate::zone::ZoneRoleManager;

use futures::future::{AbortHandle, AbortRegistration, Abortable};
use std::sync::Arc;
use std::time::Duration;

// const SYNC_PING_INTERVAL_IN_SECS: u64 = 60;
// const SYNC_PING_DURATION_IN_SECS: Duration = Duration::from_secs(SYNC_PING_INTERVAL_IN_SECS);
const SYNC_PING_TIMEOUT_IN_SECS: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub(super) struct PingResult {
    pub last_success_ping_time: u64,
    pub last_ping_result: BuckyErrorCode,
    pub last_ping_time: u64,
    pub retry_count: u32,
}

struct SyncState {
    target: DeviceSyncState,
    ping_result: PingResult,
    ping_waker: Option<AbortHandle>,
    ping_waiter: Option<AbortRegistration>,
    ping_complete: Option<Vec<AbortHandle>>,
}

impl SyncState {
    fn new(target: DeviceSyncState) -> Self {
        let ping_result = PingResult {
            last_ping_result: BuckyErrorCode::Ok,
            last_ping_time: 0,
            last_success_ping_time: 0,
            retry_count: 0,
        };

        Self {
            target,
            ping_result,
            ping_waker: None,
            ping_waiter: None,
            ping_complete: None,
        }
    }
}

#[derive(Clone)]
pub(super) struct SyncPingClient {
    device_id: DeviceId,

    device_state: Arc<DeviceStateManager>,

    sync_state: Arc<Mutex<SyncState>>,

    // use to stop ping
    ping_abort_handle: Arc<Mutex<Option<AbortHandle>>>,

    requestor: Arc<SyncClientRequestor>,

    object_sync_client: Arc<ObjectSyncClient>,

    ping_status: PingStatus,

    zone_role: ZoneRole,

    role_manager: ZoneRoleManager,
}

impl SyncPingClient {
    pub async fn new(
        device_id: &DeviceId,
        device_state: Arc<DeviceStateManager>,
        requestor: Arc<SyncClientRequestor>,
        object_sync_client: Arc<ObjectSyncClient>,
        zone_role: ZoneRole,
        role_manager: ZoneRoleManager,
    ) -> BuckyResult<Self> {
        let state = SyncState::new(DeviceSyncState::Offline);

        let ret = Self {
            device_id: device_id.clone(),
            device_state,
            sync_state: Arc::new(Mutex::new(state)),
            ping_abort_handle: Arc::new(Mutex::new(None)),
            requestor,
            object_sync_client,
            ping_status: PingStatus::new(),
            zone_role,
            role_manager,
        };

        Ok(ret)
    }

    pub fn fill_ood_status(&self, status: &mut OODStatus) {
        self.ping_status.fill_ood_status(status)
    }

    fn state(&self) -> DeviceSyncState {
        self.sync_state.lock().unwrap().target.clone()
    }

    fn update_state(&self, state: DeviceSyncState) -> DeviceSyncState {
        let mut cur_state = self.sync_state.lock().unwrap();
        let old = cur_state.target.clone();
        info!("device sync state changed: {} -> {}", old, state);

        cur_state.target = state;

        old
    }

    pub fn start(&self, state: DeviceSyncState) {
        let old = self.update_state(state);
        assert_eq!(old, DeviceSyncState::Offline);

        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        {
            let mut slot = self.ping_abort_handle.lock().unwrap();
            assert!(slot.is_none());
            *slot = Some(abort_handle);
        }

        let this = self.clone();
        async_std::task::spawn(async move {

            let fut = Abortable::new(
                this.run_ping(),
                abort_registration,
            );

            match fut.await {
                Ok(_) => {
                    error!("sync ping returned");
                }
                Err(futures::future::Aborted { .. }) => {
                    info!("sync ping aborted!");
                }
            };
        });
    }

    pub async fn stop(&self) {
        let _ = self.offline().await;

        if let Some(handle) = self.ping_abort_handle.lock().unwrap().take() {
            info!("will stop sync ping");
            handle.abort();
        } else {
            warn!("stop sync ping but not running!");
        }
    }

    async fn offline(&self) -> BuckyResult<()> {
        self.update_state(DeviceSyncState::Offline);

        self.ping_impl().await
    }

    // 立刻唤醒ping
    pub fn wakeup_ping(&self) {
        let waker = {
            let mut state = self.sync_state.lock().unwrap();
            state.ping_waker.take()
        };

        if let Some(waker) = waker {
            info!("will wakeup ping!");
            waker.abort();
        } else {
            warn!("try wakeup ping but in pinging!")
        }
    }

    pub async fn ping_state(&self, flush: bool) -> PingResult {
        if flush {
            let waker;
            let abort_registration;
            {
                let pair = AbortHandle::new_pair();
                abort_registration = pair.1;

                let mut state = self.sync_state.lock().unwrap();
                waker = state.ping_waker.take();
                if let Some(ref mut list) = state.ping_complete {
                    list.push(pair.0);
                } else {
                    state.ping_complete = Some(vec![pair.0]);
                }
            }

            if let Some(waker) = waker {
                waker.abort();
            }

            let fut = Abortable::new(
                async_std::task::sleep(SYNC_PING_TIMEOUT_IN_SECS),
                abort_registration,
            );

            match fut.await {
                Ok(_) => {
                    error!("flush ping timeout");
                }
                Err(futures::future::Aborted { .. }) => {
                    info!("flush ping wakeup");
                }
            };
        }

        let state = self.sync_state.lock().unwrap();
        state.ping_result.clone()
    }

    fn get_ping_interval(&self) -> u64 {
        if self.zone_role.is_ood_device() {
            30
        } else {
            60
        }
    }

    async fn run_ping(&self) {
        let mut failed_retry_interval_in_secs = 5;
        let mut failed_count = 0;
        let mut notified = false;   // notify role manager update zone info on continuous failure， only once before stopped
        let ping_interval = self.get_ping_interval();
        let ping_duration = std::time::Duration::from_secs(ping_interval);

        loop {
            let interval_in_secs;
            match self.ping_once().await {
                BuckyErrorCode::Ok => {
                    interval_in_secs = ping_duration;
                    failed_retry_interval_in_secs = 5;
                    failed_count = 0;
                }

                _ => {
                    // 失败后需要逐步增加间隔重试
                    interval_in_secs = Duration::from_secs(failed_retry_interval_in_secs);
                    failed_retry_interval_in_secs *= 2;
                    if failed_retry_interval_in_secs > ping_interval {
                        failed_retry_interval_in_secs = ping_interval;
                    }
                    failed_count += 1;
                }
            }

            if !notified && failed_count >= 5 {
                warn!("device ping continuous failed for {} times, now will flush zone's owner info!", failed_count);
                notified = true;
                let _ = self.role_manager.notify_owner_changed().await;
            }

            let abort_registration;

            {
                let mut state = self.sync_state.lock().unwrap();
                assert!(state.ping_waiter.is_some());
                abort_registration = state.ping_waiter.take().unwrap();
            }

            let fut =
                Abortable::new(async_std::task::sleep(interval_in_secs), abort_registration);
            match fut.await {
                Ok(_) => {
                    debug!("ping wait timeout, now will ping once");
                }
                Err(futures::future::Aborted { .. }) => {
                    debug!("ping wait waked up, now will ping once");
                }
            };
        }
    }

    async fn ping_once(&self) -> BuckyErrorCode {
        let err =
            match async_std::future::timeout(SYNC_PING_TIMEOUT_IN_SECS, self.ping_impl()).await {
                Ok(ret) => {
                    if let Err(e) = ret {
                        error!("device sync ping failed! {}", e);
                        e.code()
                    } else {
                        BuckyErrorCode::Ok
                    }
                }
                Err(async_std::future::TimeoutError { .. }) => {
                    error!("device sync ping timeout!");
                    BuckyErrorCode::Timeout
                }
            };

        // 更新ping结果
        let notify_list;
        {
            // 为了避免不一致，所以需在这里提前申请好waiter和waker
            let (abort_handle, abort_registration) = AbortHandle::new_pair();

            let mut state = self.sync_state.lock().unwrap();
            state.ping_waker = Some(abort_handle);
            state.ping_waiter = Some(abort_registration);
            notify_list = state.ping_complete.take();

            let ping_result = &mut state.ping_result;
            ping_result.last_ping_time = bucky_time_now();
            ping_result.last_ping_result = err;
            if err == BuckyErrorCode::Ok {
                ping_result.last_success_ping_time = ping_result.last_ping_time.clone();
                ping_result.retry_count = 0;
            } else {
                ping_result.retry_count += 1;
            }
        }

        // 如果ping成功了，那么尝试唤醒一次object同步服务，
        // 如果object同步刚好处于同步失败的重试等待区间，可以减少等待
        if err == BuckyErrorCode::Ok {
            self.object_sync_client.wakeup_sync();
        }

        // 唤醒所有的flush等待
        if let Some(list) = notify_list {
            for handle in list {
                handle.abort();
            }
        }

        err
    }

    async fn ping_impl(&self) -> BuckyResult<()> {
        let state = self.device_state.get_device_state().await?;

        let req = SyncPingRequest {
            device_id: self.device_id.clone(),
            zone_role: state.zone_role,
            root_state: state.root_state,
            root_state_revision: state.root_state_revision,
            state: self.state(),
            owner_update_time: state.owner_update_time,
        };

        let resp = self.requestor.ping(req, &self.ping_status).await?;

        if let Some(object_raw) = resp.owner {
            let _ = self.update_owner(object_raw).await;
        }

        let zone_state = LocalZoneState {
            zone_root_state: Some(resp.zone_root_state),
            zone_root_state_revision: resp.zone_root_state_revision,
            zone_role: resp.zone_role,
            ood_work_mode: resp.ood_work_mode,
        };

        self.device_state.update_zone_state(zone_state);

        Ok(())
    }

    pub(super) async fn update_owner(&self, object_raw: Vec<u8>) -> BuckyResult<()> {
        let owner = AnyNamedObject::clone_from_slice(&object_raw)?;
        let owner_id = owner.object_id();

        let object_info = NONObjectInfo::new(owner_id, object_raw, Some(Arc::new(owner)));
        self.role_manager.on_update_owner(&object_info).await
    }
}
