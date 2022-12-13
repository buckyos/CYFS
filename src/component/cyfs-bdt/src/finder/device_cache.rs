use std::{
    sync::{RwLock}, 
    collections::{BTreeSet, hash_map::HashMap}
};
use cyfs_base::*;
use crate::dht::*;
use super::outer_device_cache::*;

pub struct DeviceCache {
    outer: Option<Box<dyn OuterDeviceCache>>,
    //FIXME 先简单干一个
    cache: RwLock<HashMap<DeviceId, Device>>,
    // sn
    sn_list: RwLock<BTreeSet<DeviceId>>,
}

impl DeviceCache {
    pub fn new(outer: Option<Box<dyn OuterDeviceCache>>) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            outer,
            sn_list: RwLock::new(BTreeSet::new()),
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
        } else if let Some(outer) = &self.outer {
            outer.get(id).await
        } else {
            None
        }
    }
}

impl DeviceCache {
    pub fn add_sn(&self, sn_list: &Vec<Device>) {
        for sn in sn_list {
            let id = sn.desc().device_id();
            self.add(&id, sn);
            self.sn_list.write().unwrap().insert(id);
        }
       
    }

    pub fn nearest_sn_of(&self, remote: &DeviceId) -> Option<DeviceId> {
        self.sn_list.read().unwrap().iter().min_by(|l, r| l.object_id().distance(remote.object_id()).cmp(&r.object_id().distance(remote.object_id()))).cloned()
    }

    pub fn sn_list(&self) -> Vec<DeviceId> {
        self.sn_list.read().unwrap().iter().cloned().collect()
    }
}
