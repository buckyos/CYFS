use crate::root_state_api::GlobalStateLocalService;
use crate::zone::ZoneManager;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

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
}

impl std::fmt::Display for DeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{},{},{},{}",
            self.root_state, self.root_state_revision, self.zone_role, self.ood_work_mode
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
    zone_manager: ZoneManager,

    state: NOCCollectionSync<DeviceLocalState>,

    event: Box<dyn DeviceStateManagerEvent>,

    noc: Box<dyn NamedObjectCache>,
}

impl DeviceStateManager {
    pub fn new(
        device_id: &DeviceId,
        noc: Box<dyn NamedObjectCache>,
        root_state: GlobalStateLocalService,
        zone_manager: ZoneManager,
        event: Box<dyn DeviceStateManagerEvent>,
    ) -> Self {
        let id = format!("device-sync-state-{}", device_id.to_string());
        let state = NOCCollectionSync::new(&id, noc.clone_noc());

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

    pub async fn update_owner(&self, object_raw: Vec<u8>) -> BuckyResult<()> {
        let owner = AnyNamedObject::clone_from_slice(&object_raw)?;
        let owner_id = owner.object_id();

        let current_zone_info = self.zone_manager.get_current_info().await?;
        let current_owner_id = current_zone_info.owner.object_id();
        if owner_id != current_owner_id {
            let msg = format!(
                "device update owner but id unmatch! current={}, got={}",
                current_owner_id, owner_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        // save to noc...
        let req = NamedObjectCacheInsertObjectRequest {
            protocol: NONProtocol::Native,
            source: self.device_id.clone(),
            object_id: owner_id.clone(),
            dec_id: None,
            object: Arc::new(owner),
            object_raw,
            flags: 0,
        };

        match self.noc.insert_object(&req).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCacheInsertResult::Accept
                    | NamedObjectCacheInsertResult::Updated => {
                        info!("device update owner object to noc success: {}", owner_id);
                    }
                    NamedObjectCacheInsertResult::AlreadyExists => {
                        warn!(
                            "device update owner object but already exists: {}",
                            owner_id
                        );
                    }
                    NamedObjectCacheInsertResult::Merged => {
                        warn!(
                            "device update owner object but signs merged success: {}",
                            owner_id
                        );
                    }
                }

                Ok(())
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
}
