use super::validate::GlobalStateValidator;
use crate::root_state_api::GlobalStateLocalService;
use cyfs_lib::*;
use cyfs_base::*;

#[derive(Clone)]
pub struct GlobalStateValidatorManager {
    root_state: GlobalStateValidator,
    local_cache: GlobalStateValidator,
}

impl GlobalStateValidatorManager {
    pub fn new(device_id: &DeviceId, root_state: &GlobalStateLocalService, local_cache: &GlobalStateLocalService) -> Self {
        Self {
            root_state: GlobalStateValidator::new(device_id.clone(), root_state.state().clone()),
            local_cache: GlobalStateValidator::new(device_id.clone(), local_cache.state().clone()),
        }
    }

    pub fn get_validator(&self, category: GlobalStateCategory) -> &GlobalStateValidator {
        match category {
            GlobalStateCategory::RootState => &self.root_state,
            GlobalStateCategory::LocalCache => &self.local_cache,
        }
    }
}
