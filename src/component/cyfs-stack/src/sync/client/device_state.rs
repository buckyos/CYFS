use crate::root_state_api::GlobalStateLocalService;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

#[derive(RawEncode, RawDecode, Debug, Clone, Eq, PartialEq)]
pub struct LocalZoneState {
    // 当前zone的最新root state
    pub zone_root_state: Option<ObjectId>,
    pub zone_root_state_revision: u64,
    pub zone_role: ZoneRole,
    pub ood_work_mode: OODWorkMode,
}

impl Default for LocalZoneState {
    fn default() -> Self {
        Self {
            zone_root_state: None,
            zone_root_state_revision: 0,
            zone_role: ZoneRole::ActiveOOD,
            ood_work_mode: OODWorkMode::Standalone,
        }
    }
}

impl std::fmt::Display for LocalZoneState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?},{},{},{}",
            self.zone_root_state, self.zone_root_state_revision, self.zone_role, self.ood_work_mode
        )
    }
}

pub(crate) struct DeviceState {
    pub root_state: ObjectId,
    pub root_state_revision: u64,
    pub zone_role: ZoneRole,
    pub ood_work_mode: OODWorkMode,
    pub owner_update_time: u64,
}

impl std::fmt::Display for DeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{},{},{},{},{}",
            self.root_state,
            self.root_state_revision,
            self.zone_role,
            self.ood_work_mode,
            self.owner_update_time,
        )
    }
}

#[derive(RawEncode, RawDecode, Debug, Clone)]
pub(crate) struct DeviceLocalState {
    // zone latest state
    zone_state: LocalZoneState,
}

impl Default for DeviceLocalState {
    fn default() -> Self {
        Self {
            zone_state: LocalZoneState::default(),
        }
    }
}

pub(crate) trait DeviceStateManagerEvent: Sync + Send + 'static {
    fn zone_state_update(&self, old_zone_state: LocalZoneState, new_zone_state: LocalZoneState);
}

pub(crate) struct DeviceStateManager {
    device_id: DeviceId,

    root_state: GlobalStateLocalService,
    zone_manager: ZoneManagerRef,

    state: NOCCollectionSync<DeviceLocalState>,

    event: Box<dyn DeviceStateManagerEvent>,

    noc: NamedObjectCacheRef,
}

impl DeviceStateManager {
    pub fn new(
        device_id: &DeviceId,
        noc: NamedObjectCacheRef,
        root_state: GlobalStateLocalService,
        zone_manager: ZoneManagerRef,
        event: Box<dyn DeviceStateManagerEvent>,
    ) -> Self {
        let id = format!("device-sync-state-{}", device_id.to_string());
        let state = NOCCollectionSync::new(&id, noc.clone());

        Self {
            device_id: device_id.to_owned(),
            root_state,
            zone_manager,
            state,
            event,
            noc,
        }
    }

    pub async fn load(&self) -> BuckyResult<()> {
        match self.state.load().await {
            Ok(_) => {
                info!(
                    "load device sync state success! state={:?}",
                    self.state.coll().lock().unwrap()
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "load device sync state failed! now will treat as init state: {:?}, err={}",
                    self.state.coll().lock().unwrap(),
                    e
                );
                Err(e)
            }
        }
    }

    pub fn start(&self) {
        let interval = std::time::Duration::from_secs(15);
        info!("device sync state start auto save: {:?}", interval);
        self.state.start_save(interval);
    }

    // get current device's local state dynamically
    pub async fn get_device_state(&self) -> BuckyResult<DeviceState> {
        let (root_state, root_state_revision) = self.root_state.state().get_current_root();
        let current_info = self.zone_manager.get_current_info().await?;
        let state = DeviceState {
            root_state,
            root_state_revision,
            ood_work_mode: current_info.ood_work_mode.clone(),
            zone_role: current_info.zone_role.clone(),
            owner_update_time: current_info.owner.get_update_time(),
        };

        Ok(state)
    }

    pub fn get_zone_state(&self) -> LocalZoneState {
        let coll = self.state.coll().lock().unwrap();
        /*
        info!(
            "current zone state: device={}, {}",
            self.device_id, coll.zone_state
        );
        */
        coll.zone_state.clone()
    }

    pub fn update_zone_state(&self, zone_state: LocalZoneState) {
        let old_zone_state;
        let new_zone_state;
        {
            let mut coll = self.state.coll().lock().unwrap();

            old_zone_state = coll.zone_state.clone();
            new_zone_state = zone_state.clone();

            if old_zone_state != new_zone_state {
                info!(
                    "zone state updated: {} -> {}",
                    old_zone_state, new_zone_state
                );

                coll.zone_state = zone_state;

                self.state.set_dirty(true);
            }
        }

        /*
        info!(
            "current zone state: device={}, {}",
            self.device_id,
            self.get_zone_state()
        );
        */

        if old_zone_state != new_zone_state {
            self.event.zone_state_update(old_zone_state, new_zone_state);
        }
    }
}
