use std::{
    sync::{RwLock}, 
    collections::{BTreeSet, BTreeMap}
};
use cyfs_base::*;
use crate::{
    types::*, 
    dht::*
};

pub struct SnCache {
    known_list: RwLock<BTreeSet<DeviceId>>, 
    active_endpoints: RwLock<BTreeMap<DeviceId, EndpointPair>> 
}

impl SnCache {
    pub fn new() -> Self {
        Self {
            known_list: RwLock::new(BTreeSet::new()),
            active_endpoints: RwLock::new(BTreeMap::new())
        }
    }

    pub fn add_known_sn(&self, sn_list: &Vec<DeviceId>) {
        let mut known_list = self.known_list.write().unwrap();
        for sn in sn_list {
            known_list.insert(sn.clone());
        }
       
    }
    pub fn nearest_sn_of(remote: &DeviceId, sn_list: &[DeviceId]) -> Option<DeviceId> {
        sn_list.iter().min_by(|l, r| l.object_id().distance(remote.object_id()).cmp(&r.object_id().distance(remote.object_id()))).cloned()
    }

    pub fn known_list(&self) -> Vec<DeviceId> {
        self.known_list.read().unwrap().iter().cloned().collect()
    }

    pub fn add_active(&self, sn: &DeviceId, active: EndpointPair) {
        self.active_endpoints.write().unwrap().insert(sn.clone(), active);
    }

    pub fn get_active(&self, sn: &DeviceId) -> Option<EndpointPair> {
        self.active_endpoints.read().unwrap().get(sn).cloned()
    }

    pub fn remove_active(&self, sn: &DeviceId) {
        self.active_endpoints.write().unwrap().remove(sn);
    }
}
