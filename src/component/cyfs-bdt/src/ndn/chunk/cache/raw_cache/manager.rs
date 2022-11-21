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
    pub fn new(config: RawCacheConfig) -> Self {
        Self(Arc::new(ManagerImpl {
            config, 
            state: RwLock::new(ManagerState {
                total_mem: 0
            })
        }))
    }

    pub fn config(&self) -> &RawCacheConfig {
        &self.0.config
    }

    pub async fn alloc(&self, capacity: usize) -> Box<dyn RawCache> {
        // FIXME: create file cache when outof config 
        self.alloc_mem(capacity)
    }

    pub fn alloc_mem(&self, capacity: usize) -> Box<dyn RawCache> {
        self.0.state.write().unwrap().total_mem += capacity as u64;
        MemCacheGuard::new(self.clone(), capacity).clone_as_raw_cache()
    }

    pub(super) fn release_mem(&self, capacity: usize) {
        self.0.state.write().unwrap().total_mem -= capacity as u64;
    }
}