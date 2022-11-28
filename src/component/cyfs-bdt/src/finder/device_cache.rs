use std::{
    sync::{RwLock}, 
    collections::hash_map::HashMap
};
use cyfs_base::*;
use crate::dht::*;
use super::outer_device_cache::*;

pub struct DeviceCache {
    local_id: DeviceId,
    local: RwLock<Device>,
    outer: Option<Box<dyn OuterDeviceCache>>,
    //FIXME 先简单干一个
    cache: RwLock<HashMap<DeviceId, Device>>,
    // sn
    sn_list: RwLock<Vec<DeviceId>>,
}

impl DeviceCache {
    pub fn new(local: Device, outer: Option<Box<dyn OuterDeviceCache>>) -> Self {
        Self {
            local_id: local.desc().device_id(),
            local: RwLock::new(local),
            cache: RwLock::new(HashMap::new()),
            outer,
            sn_list: RwLock::new(vec![]),
        }
    }

    pub fn local(&self) -> Device {
        let local = self.local.read().unwrap();
        (&*local).clone()
    }

    pub fn update_local(&self, desc: &Device) {
        let mut local = self.local.write().unwrap();
        *local = desc.clone();
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

        // 临时方案：这里需要添加到上层noc
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
        let mem_cache = self.cache.read().unwrap().get(id).map(|d| d.clone());
        if mem_cache.is_some() {
            mem_cache
        } else if self.local_id.eq(id) {
            Some(self.local())
        } else if let Some(outer) = &self.outer {
            outer.get(id).await
        } else {
            None
        }
    }
}

impl DeviceCache {
    pub fn reset_sn_list(&self, sn_list: &Vec<Device>) {
        let id_list: Vec<DeviceId> = sn_list.iter().map(|d| d.desc().device_id()).collect();
        for (id, device) in id_list.iter().zip(sn_list.iter()) {
            self.add(id, device);
        }
        *self.sn_list.write().unwrap() = id_list;
    }

    pub fn nearest_sn_of(&self, remote: &DeviceId) -> Option<DeviceId> {
        self.sn_list.read().unwrap().iter().min_by(|l, r| l.object_id().distance(remote.object_id()).cmp(&r.object_id().distance(remote.object_id()))).cloned()
    }

    pub fn sn_list(&self) -> Vec<DeviceId> {
        self.sn_list.read().unwrap().clone()
    }
}
