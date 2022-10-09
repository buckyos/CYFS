use std::{
    sync::{Arc, RwLock}
};
use super::{
    common::*,
    mem::*
};


struct ManagerState {
    total_mem: u64, 
}

struct ManagerImpl {
    config: RawCacheConfig, 
    state: RwLock<ManagerState>
}

#[derive(Clone)]
pub struct RawCacheManager(Arc<ManagerImpl>);

impl RawCacheManager {
    pub fn new() -> Self {
        Self(Arc::new(ManagerImpl {
            config: RawCacheConfig {}, 
            state: RwLock::new(ManagerState {
                total_mem: 0
            })
        }))
    }

    pub async fn alloc(&self, capacity: usize) -> Box<dyn RawCache> {
        unimplemented!()
    }

    pub fn alloc_mem(&self, capacity: usize) -> Box<dyn RawCache> {
        unimplemented!()
    }

    pub(super) fn release_mem(&self, capacity: usize) {
        unimplemented!()
    }
}