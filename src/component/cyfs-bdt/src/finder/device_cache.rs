use std::{
    sync::{RwLock}, 
    collections::{hash_map::HashMap}
};
use cyfs_base::*;
use super::outer_device_cache::*;

pub struct DeviceCache {
    outer: Option<Box<dyn OuterDeviceCache>>,
    //FIXME 先简单干一个
    cache: RwLock<HashMap<DeviceId, Device>>,
}

impl DeviceCache {
    pub fn new(outer: Option<Box<dyn OuterDeviceCache>>) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            outer,
        }
    }

    pub fn add(&self, id: &DeviceId, device: &Device) {
        // FIXME 这里添加一个检测，确保添加的device id匹配
        let real_device_id = device.desc().device_id();
        if *id != real_device_id {
            error!("add device but unmatch device_id! param_id={}, calc_id={}", id, real_device_id);
            // panic!("{}", msg);
            return;
        }


        // 添加到内存缓存
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(id.clone(), device.clone());
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
        self.cache.read().unwrap().get(id).cloned()
    }

    pub fn remove_inner(&self, id: &DeviceId) {
        self.cache.write().unwrap().remove(id);
    }
}
