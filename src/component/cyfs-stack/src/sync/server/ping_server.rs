use super::super::protocol::*;
use super::zone_state::ZoneStateManager;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_lib::ZoneRole;


use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;


const SYNC_PING_INTERVAL_IN_SECS: u64 = 60;
const SYNC_PING_TIMEOUT_IN_SECS: u64 = SYNC_PING_INTERVAL_IN_SECS * 3;

#[derive(Clone, Debug)]
struct DevicePingState {
    latest_ping: u64,
    count: u32,

    state: DeviceSyncState,
}

struct SyncPingServerState {
    timeout: u64,
    device_list: HashMap<DeviceId, DevicePingState>,
}

impl SyncPingServerState {
    pub fn new() -> Self {
        let timeout = Duration::from_secs(SYNC_PING_TIMEOUT_IN_SECS).as_micros() as u64;
        Self {
            timeout,
            device_list: HashMap::new(),
        }
    }

    fn check_timeout(&mut self) -> Vec<(DeviceId, DevicePingState)> {
        let now = bucky_time_now();
        let timeout = self.timeout.clone();
        let mut device_list = Vec::new();

        self.device_list.retain(|device_id, state| {
            if now - state.latest_ping >= timeout {
                warn!(
                    "device timeout: {}, last ping={}",
                    device_id, state.latest_ping
                );

                device_list.push((device_id.to_owned(), state.clone()));
                false
            } else {
                true
            }
        });

        device_list
    }
}

#[derive(Clone)]
pub(crate) struct SyncPingServer {
    state: Arc<Mutex<SyncPingServerState>>,
    zone_state: Arc<ZoneStateManager>,
}

#[derive(Debug)]
pub(crate) struct SyncDeviceInfo {
    pub device_id: DeviceId,
}

impl SyncPingServer {
    pub fn new(zone_state: Arc<ZoneStateManager>) -> Self {
        let state = SyncPingServerState::new();
        let state = Arc::new(Mutex::new(state));

        Self { state, zone_state }
    }

    pub fn start(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            this.run_checker().await;
        });
    }

    // 获取当前可同步的设备列表
    pub fn sync_device_list(&self) -> Vec<SyncDeviceInfo> {
        let state = self.state.lock().unwrap();
        let info_list: Vec<SyncDeviceInfo> = state
            .device_list
            .iter()
            .filter_map(|(device_id, device_state)| {
                if device_state.state == DeviceSyncState::OnlineAccept {
                    let info = SyncDeviceInfo {
                        device_id: device_id.to_owned(),
                    };

                    Some(info)
                } else {
                    None
                }
            })
            .collect();

        info_list
    }

    pub fn ping(&self, ping_req: &SyncPingRequest) -> BuckyResult<SyncPingResponse> {
        let (zone_state, _device_state) = match ping_req.state {
            DeviceSyncState::Online | DeviceSyncState::OnlineAccept => {
                let mut state = self.state.lock().unwrap();
                match state.device_list.get_mut(&ping_req.device_id) {
                    Some(device_state) => {
                        // 非首次ping
                        match self.zone_state.device_update(&ping_req) {
                            Ok(ret) => {
                                device_state.count += 1;
                                device_state.state = ping_req.state.clone();
                                device_state.latest_ping = bucky_time_now();

                                ret
                            }
                            Err(e) => {
                                warn!(
                                    "will remove device from ping state: device={}, zone_role={}, {}",
                                    ping_req.device_id, ping_req.zone_role, e
                                );
                                state.device_list.remove(&ping_req.device_id);

                                return Err(e);
                            }
                        }
                    }
                    None => {
                        // 首次ping
                        match self.zone_state.device_online(&ping_req) {
                            Ok(ret) => {
                                info!(
                                    "device online success! device={}, zone_role={}",
                                    ping_req.device_id, ping_req.zone_role
                                );

                                let device_state = DevicePingState {
                                    latest_ping: bucky_time_now(),
                                    state: ping_req.state.clone(),
                                    count: 1,
                                };
                                state
                                    .device_list
                                    .insert(ping_req.device_id.clone(), device_state);
                                ret
                            }
                            Err(e) => {
                                error!(
                                    "device online failed! device={}, zone_role={}, {}",
                                    ping_req.device_id, ping_req.zone_role, e
                                );
                                return Err(e);
                            }
                        }
                    }
                }
            }

            DeviceSyncState::Offline => {
                // 从ping state里面移除
                {
                    let mut state = self.state.lock().unwrap();
                    match state.device_list.remove(&ping_req.device_id) {
                        Some(device_state) => {
                            info!(
                                "device offline: device={}, zone_role={}, ping_state={:?}",
                                ping_req.device_id, ping_req.zone_role, device_state,
                            );
                        }

                        None => {
                            error!(
                                "device offline but not found! device={}, zone_role={}",
                                ping_req.device_id, ping_req.zone_role,
                            );
                        }
                    }
                }

                // 下线操作
                self.zone_state.device_offline(&ping_req)?
            }
        };

        let mut resp = SyncPingResponse {
            zone_root_state: zone_state.zone_root_state,
            zone_root_state_revision: zone_state.zone_root_state_revision,
            ood_work_mode: zone_state.ood_work_mode,
            zone_role: zone_state.zone_role,
            owner: None,
        };

        if resp.zone_role != ZoneRole::ActiveOOD {
            warn!("recv device ping but current ood' role is not active ood! role={}", resp.zone_role);
            let object_raw = zone_state.owner.to_vec().unwrap();
            resp.owner = Some(object_raw);
        }

        Ok(resp)
    }

    fn check_timeout(&self) {
        let list = self.state.lock().unwrap().check_timeout();

        for (device_id, _state) in list {
            match self.zone_state.get_zone_device_state(&device_id) {
                Some(device_state) => {
                    let req = SyncPingRequest {
                        device_id,
                        zone_role: device_state.zone_role,
                        root_state: device_state.root_state,
                        root_state_revision: device_state.root_state_revision,
                        state: DeviceSyncState::Offline,
                    };

                    let _r = self.zone_state.device_offline(&req);
                }
                None => {
                    error!(
                        "device ping timeout offline but not found in device list: {}",
                        device_id
                    );
                }
            }
        }
    }

    async fn run_checker(self) {
        use async_std::prelude::*;

        let mut interval = async_std::stream::interval(Duration::from_secs(30));
        while let Some(_) = interval.next().await {
            self.check_timeout();
        }
    }
}
