use super::super::protocol::*;
use crate::root_state_api::*;
use crate::zone::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;

pub(crate) struct Device;

#[derive(RawEncode, RawDecode, Debug, Clone)]
pub(crate) struct ZoneDeviceState {
    // device's current state
    pub root_state: ObjectId,
    pub root_state_revision: u64,
    pub zone_role: ZoneRole,

    // 最后一次上线时间
    pub last_online: u64,

    // 最后一次下线时间
    pub last_offline: u64,
}

impl Default for ZoneDeviceState {
    fn default() -> Self {
        Self {
            root_state: ObjectId::default(),
            root_state_revision: 0,
            zone_role: ZoneRole::Device,
            last_online: 0,
            last_offline: 0,
        }
    }
}

impl ZoneDeviceState {
    pub fn is_changed(&self, ping_req: &SyncPingRequest) -> bool {
        self.root_state != ping_req.root_state
            || self.root_state_revision != ping_req.root_state_revision
            || self.zone_role != ping_req.zone_role
    }

    pub fn update(&mut self, ping_req: &SyncPingRequest) {
        self.root_state = ping_req.root_state.clone();
        self.root_state_revision = ping_req.root_state_revision.clone();
        self.zone_role = ping_req.zone_role.clone();
    }
}

//#[derive(Eq, PartialEq)]
pub struct ZoneState {
    // 当前zone的最新root state
    pub zone_root_state: ObjectId,
    pub zone_root_state_revision: u64,
    pub zone_role: ZoneRole,
    pub ood_work_mode: OODWorkMode,

    // 当前zone对应的owner对象
    pub owner: Arc<AnyNamedObject>,
}

impl std::fmt::Display for ZoneState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{},{},{},{}",
            self.zone_root_state, self.zone_root_state_revision, self.zone_role, self.ood_work_mode,
        )
    }
}

#[derive(RawEncode, RawDecode, Debug, Clone)]
struct ZoneInfo {
    device_list: HashMap<DeviceId, ZoneDeviceState>,
}

impl Default for ZoneInfo {
    fn default() -> Self {
        Self {
            device_list: HashMap::new(),
        }
    }
}

struct ZoneChangedNotify {
    owner: ZoneStateManager,
}

impl EventListenerSyncRoutine<ZoneId, ()> for ZoneChangedNotify {
    fn call(&self, param: &ZoneId) -> BuckyResult<()> {
        if *param == self.owner.zone_id {
            // 只关注当前zone改变
            self.owner.on_zone_changed();
        }

        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct ZoneStateManager {
    zone_id: ZoneId,

    zone_manager: ZoneManagerRef,
    root_state: GlobalStateLocalService,

    state: NOCCollectionSync<ZoneInfo>,
}

impl ZoneStateManager {
    pub fn new(
        zone_id: &ZoneId,
        root_state: GlobalStateLocalService,
        zone_manager: ZoneManagerRef,
        noc: NamedObjectCacheRef,
    ) -> Self {
        let id = format!("zone-sync-state-{}", zone_id.to_string());
        let state = NOCCollectionSync::new(&id, noc);

        let ret = Self {
            zone_id: zone_id.to_owned(),
            root_state,
            zone_manager,
            state,
        };

        // 关注zone更新事件
        let notify = ZoneChangedNotify { owner: ret.clone() };
        ret.zone_manager.zone_changed_event().on(Box::new(notify));

        ret
    }

    pub async fn load(&self) -> BuckyResult<()> {
        let ret = match self.state.load().await {
            Ok(_) => {
                info!(
                    "load zone sync state success! state={:?}",
                    self.state.coll().lock().unwrap()
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "load zone sync state failed! now will treat as init state: {:?}, err={}",
                    self.state.coll().lock().unwrap(),
                    e
                );
                Err(e)
            }
        };

        self.zone_manager.get_current_zone().await?;

        // 加载当前zone信息
        self.on_zone_changed();

        ret
    }

    pub fn start(&self) {
        let interval = std::time::Duration::from_secs(60);
        info!("zone sync state start auto save: {:?}", interval);
        self.state.start_save(interval);
    }

    pub async fn verify_source(&self, source: &DeviceId) -> BuckyResult<()> {
        let zone = self.zone_manager.get_zone(source, None).await?;
        if zone.zone_id() != self.zone_id {
            let msg = format!(
                "zone not match: source zone={}, ood zone={}",
                zone.zone_id(),
                self.zone_id
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotMatch, msg));
        }

        Ok(())
    }

    pub async fn get_zone_state(&self) -> ZoneState {
        let (zone_root_state, zone_root_state_revision) =
            self.root_state.state().get_current_root();

        let current_zone_info = self.zone_manager.get_current_info().await.unwrap();
        let state = ZoneState {
            zone_root_state,
            zone_root_state_revision,
            ood_work_mode: current_zone_info.ood_work_mode.clone(),
            zone_role: current_zone_info.zone_role.clone(),
            owner: current_zone_info.owner.clone(),
        };

        state
    }

    fn on_zone_changed(&self) {
        let ret = self.zone_manager.query(&self.zone_id);
        if ret.is_none() {
            error!("zone not found in zone manager! {}", self.zone_id);
            return;
        }

        info!("zone changed, now will update state: {}", self.zone_id);
        let zone = ret.unwrap();
        self.update_zone(&zone);
    }

    // 一个zone的device发生了更新
    fn update_zone(&self, zone: &Zone) {
        let mut zone_state = self.state.coll().lock().unwrap();

        // 查找新增的device
        for device_id in zone
            .known_device_list()
            .iter()
            .chain(zone.ood_list().iter())
        {
            match zone_state.device_list.entry(device_id.to_owned()) {
                Entry::Vacant(v) => {
                    info!(
                        "new device in zone: zone={}, device={}",
                        self.zone_id, device_id
                    );

                    let new_device = ZoneDeviceState::default();
                    v.insert(new_device);
                }
                Entry::Occupied(_) => {}
            }
        }

        // 找到被移除的device
        zone_state
            .device_list
            .retain(|device_id, _| match zone.is_known_device(device_id) {
                true => true,
                false => {
                    warn!(
                        "device had been remove from zone! zone={}, device={}",
                        self.zone_id, device_id
                    );
                    false
                }
            });

        self.state.set_dirty(true);
    }

    pub fn device_online(&self, ping_req: &SyncPingRequest) -> BuckyResult<ZoneDeviceState> {
        let ret = {
            let mut state = self.state.coll().lock().unwrap();

            let device_state = {
                let device_state =
                    state
                        .device_list
                        .get_mut(&ping_req.device_id)
                        .ok_or_else(|| {
                            let msg = format!(
                                "device not found in zone! zone={}, device={}, zone_role={}",
                                self.zone_id, ping_req.device_id, ping_req.zone_role,
                            );
                            error!("{}", msg);

                            BuckyError::new(BuckyErrorCode::PermissionDenied, msg)
                        })?;

                device_state.last_online = bucky_time_now();
                device_state.update(ping_req);

                device_state.clone()
            };

            device_state
        };

        info!("device state change to online: {:?}", ret);
        self.state.set_dirty(true);

        Ok(ret)
    }

    pub fn device_offline(&self, ping_req: &SyncPingRequest) -> BuckyResult<ZoneDeviceState> {
        let device_state = match self
            .state
            .coll()
            .lock()
            .unwrap()
            .device_list
            .get_mut(&ping_req.device_id)
        {
            Some(device_state) => {
                device_state.last_offline = bucky_time_now();
                device_state.update(ping_req);

                info!(
                    "device offline from zone state: {}, root_state={}, zone_role={}, online={}",
                    ping_req.device_id,
                    device_state.root_state,
                    device_state.zone_role,
                    device_state.last_online,
                );

                device_state.clone()
            }
            None => {
                let msg = format!(
                    "device offline but not found! {}, {}",
                    ping_req.device_id, ping_req.zone_role
                );
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        };

        info!(
            "device state change to offline: {}, {}",
            ping_req.device_id, ping_req.zone_role
        );
        self.state.set_dirty(true);

        Ok(device_state)
    }

    pub fn device_update(&self, ping_req: &SyncPingRequest) -> BuckyResult<ZoneDeviceState> {
        let mut changed = false;
        let mut zone_state = self.state.coll().lock().unwrap();
        let device_state = match zone_state.device_list.get_mut(&ping_req.device_id) {
            Some(device_state) => {
                if device_state.is_changed(ping_req) {
                    info!(
                        "zone device sync state update: device={}, root_state: {} -> {}, {}",
                        ping_req.device_id,
                        device_state.root_state,
                        ping_req.root_state,
                        ping_req.zone_role
                    );
                    device_state.update(ping_req);

                    changed = true;
                }

                device_state.clone()
            }
            None => {
                let msg = format!(
                    "device not found in zone state: zone={}, device={}, zone_role={}",
                    self.zone_id, ping_req.device_id, ping_req.zone_role,
                );
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        };

        if changed {
            self.state.set_dirty(true);
        }

        Ok(device_state)
    }

    pub async fn save(&self) {
        match self.state.save().await {
            Ok(_) => {
                info!("zone sync state save success! zone={}", self.zone_id);
            }
            Err(e) => {
                error!("zone sync state save error! zone={}, {}", self.zone_id, e);
            }
        }
    }

    pub fn get_zone_device_state(&self, device_id: &DeviceId) -> Option<ZoneDeviceState> {
        let zone_state = self.state.coll().lock().unwrap();
        zone_state.device_list.get(&device_id).cloned()
    }
}
