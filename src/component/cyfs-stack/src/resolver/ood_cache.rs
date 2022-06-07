use cyfs_base::*;

use lru_time_cache::{Entry, LruCache};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub struct OodInfo {
    pub ood_list: Vec<DeviceId>,
}

pub(super) struct OodCache {
    list: Arc<Mutex<LruCache<ObjectId, OodInfo>>>,
}

impl OodCache {
    pub fn new() -> Self {
        let list =
            LruCache::with_expiry_duration_and_capacity(Duration::from_secs(3600 * 24 * 7), 1024);

        Self {
            list: Arc::new(Mutex::new(list)),
        }
    }

    pub fn add(&self, object_id: &ObjectId, ood_list: Vec<DeviceId>) {
        match self.list.lock().unwrap().entry(object_id.clone()) {
            Entry::Occupied(v) => {
                info!("will replace resolved ood list: {}", object_id);
                (*v.into_mut()).ood_list = ood_list;
            }
            Entry::Vacant(v) => {
                info!("will save resolved ood list: {}, {:?}", object_id, ood_list);
                let item = OodInfo { ood_list };

                v.insert(item);
            }
        }
    }

    pub fn get(&mut self, object_id: &ObjectId) -> Option<Vec<DeviceId>> {
        let mut list = self.list.lock().unwrap();
        let info = list.get(object_id);
        if info.is_none() {
            return None;
        }

        Some(info.unwrap().ood_list.clone())
    }
}
