use std::{
    sync::{Arc, RwLock}
};
use cyfs_base::*;
use super::{
    common::*,
    mem::*
};


struct ManagerState {
    total_mem: u64, 
}

struct ManagerImpl {
    local: DeviceId, 
    config: RawCacheConfig, 
    state: RwLock<ManagerState>
}

impl std::fmt::Display for RawCacheManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RawCache{{local:{}}}", self.0.local)
    }
}

#[derive(Clone)]
pub struct RawCacheManager(Arc<ManagerImpl>);

impl RawCacheManager {
    pub fn new(local: DeviceId, config: RawCacheConfig) -> Self {
        Self(Arc::new(ManagerImpl {
            local, 
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

    pub fn used_mem(&self) -> u64 {
        self.0.state.read().unwrap().total_mem
    }

    pub fn alloc_mem(&self, capacity: usize) -> Box<dyn RawCache> {
        info!("{} alloc raw cache {}", self, capacity);
        self.0.state.write().unwrap().total_mem += capacity as u64;
        MemCache::new(capacity, Some(self.clone())).clone_as_raw_cache()
    }

    pub(super) fn release_mem(&self, capacity: usize) {
        info!("{} release raw cache {}", self, capacity);
        self.0.state.write().unwrap().total_mem -= capacity as u64;
    }
}