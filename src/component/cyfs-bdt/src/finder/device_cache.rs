use std::{
    sync::{Mutex}, 
    time::Duration, 
    collections::{BTreeMap}
};
use lru_time_cache::LruCache;
use cyfs_base::*;
use super::outer_device_cache::*;

#[derive(Clone)]
pub struct DeviceCacheConfig {
    pub expire: Duration, 
    pub capacity: usize
}

struct MemCaches {
    lru_caches: LruCache<DeviceId, Device>, 
    static_caches: BTreeMap<DeviceId, Device>
}

impl MemCaches {
    fn new(config: &DeviceCacheConfig) -> Self {
        Self {
            static_caches: BTreeMap::new(), 
            lru_caches: LruCache::with_expiry_duration_and_capacity(config.expire, config.capacity)
        }
    }

    fn remove(&mut self, remote: &DeviceId) {
        self.static_caches.remove(remote);
        self.lru_caches.remove(remote);
    }

    fn get(&mut self, remote: &DeviceId) -> Option<&Device> {
        self.lru_caches.get(remote).or_else(|| self.static_caches.get(remote))
    }
}

pub struct DeviceCache {
    outer: Option<Box<dyn OuterDeviceCache>>,
    //FIXME 先简单干一个
    cache: Mutex<MemCaches>,
}

impl DeviceCache {
    pub fn new(
        config: &DeviceCacheConfig, 
        outer: Option<Box<dyn OuterDeviceCache>>
    ) -> Self {
        Self {
            cache: Mutex::new(MemCaches::new(config)),
            outer,
        }
    }

    pub fn add_static(&self, id: &DeviceId, device: &Device) {
        let real_device_id = device.desc().device_id();
        if *id != real_device_id {
            error!("add device but unmatch device_id! param_id={}, calc_id={}", id, real_device_id);
            // panic!("{}", msg);
            return;
        }

        {
            let mut cache = self.cache.lock().unwrap();
            cache.static_caches.insert(id.clone(), device.clone());
        }

        if let Some(outer) = &self.outer {
            let outer = outer.clone_cache();
            let id = id.to_owned();
            let device = device.to_owned();
            
            async_std::task::spawn(async move {
                outer.add(&id, device).await;
            });
        }
    }

    pub fn add(&self, id: &DeviceId, device: &Device) {
        let real_device_id = device.desc().device_id();
        if *id != real_device_id {
            error!("add device but unmatch device_id! param_id={}, calc_id={}", id, real_device_id);
            // panic!("{}", msg);
            return;
        }

        {
            let mut cache = self.cache.lock().unwrap();
            cache.lru_caches.insert(id.clone(), device.clone());
        }

        if let Some(outer) = &self.outer {
            let outer = outer.clone_cache();
            let id = id.to_owned();
            let device = device.to_owned();
            
            async_std::task::spawn(async move {
                outer.add(&id, device).await;
            });
        }
    }

    pub async fn get(&self, id: &DeviceId) -> Option<Device> {
        let mem_cache = self.get_inner(id);
        if mem_cache.is_some() {
            mem_cache
        } else if let Some(outer) = &self.outer {
            outer.get(id).await
        } else {
            None
        }
    }

    pub fn get_inner(&self, id: &DeviceId) -> Option<Device> {
        self.cache.lock().unwrap().get(id).cloned()
    }

    pub fn remove_inner(&self, id: &DeviceId) {
        self.cache.lock().unwrap().remove(id);
    }
}
