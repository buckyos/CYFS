use crate::stack::{CyfsStackParams, BdtStackParams};
use cyfs_lib::*;

use crossbeam::atomic::AtomicCell;
use std::ops::Deref;
use std::sync::Arc;

pub struct StackGlobalConfigInner {
    stack_params: CyfsStackParams,
    bdt_params: BdtStackParams,

    // global state access mode
    root_state_access_mode: AtomicCell<GlobalStateAccessMode>,
    local_cache_access_mode: AtomicCell<GlobalStateAccessMode>,
}

impl StackGlobalConfigInner {
    pub fn new(stack_params: CyfsStackParams, bdt_params: BdtStackParams,) -> Self {
        Self {
            stack_params,
            bdt_params,
            root_state_access_mode: AtomicCell::new(GlobalStateAccessMode::Read),
            local_cache_access_mode: AtomicCell::new(GlobalStateAccessMode::Write),
        }
    }

    pub fn get_stack_params(&self) -> &CyfsStackParams {
        &self.stack_params
    }

    pub fn get_bdt_params(&self) -> &BdtStackParams {
        &self.bdt_params
    }

    pub fn get_access_mode(&self, category: GlobalStateCategory) -> GlobalStateAccessMode {
        let state = match category {
            GlobalStateCategory::RootState => &self.root_state_access_mode,
            GlobalStateCategory::LocalCache => &self.local_cache_access_mode,
        };

        state.load()
    }

    pub fn change_access_mode(
        &self,
        category: GlobalStateCategory,
        access_mode: GlobalStateAccessMode,
    ) {
        let state = match category {
            GlobalStateCategory::RootState => &self.root_state_access_mode,
            GlobalStateCategory::LocalCache => &self.local_cache_access_mode,
        };

        let old = state.swap(access_mode);

        if old != access_mode {
            warn!(
                "global state access mode changed: category={}, {:?} -> {:?}",
                category, old, access_mode
            );
        }
    }
}

#[derive(Clone)]
pub struct StackGlobalConfig(Arc<StackGlobalConfigInner>);

impl StackGlobalConfig {
    pub fn new(stack_params: CyfsStackParams, bdt_params: BdtStackParams,) -> Self {
        Self(Arc::new(StackGlobalConfigInner::new(stack_params, bdt_params)))
    }
}

impl Deref for StackGlobalConfig {
    type Target = StackGlobalConfigInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
