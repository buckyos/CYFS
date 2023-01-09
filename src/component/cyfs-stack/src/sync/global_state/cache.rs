use cyfs_base::*;

use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;
use cyfs_debug::Mutex;

struct ObjectState {
    last_sync: u64,
}

#[derive(Clone)]
pub(crate) struct SyncObjectsStateCache {
    missing_list: Arc<Mutex<HashMap<ObjectId, ObjectState>>>,
}

impl SyncObjectsStateCache {
    pub fn new() -> Self {
        Self { missing_list: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn is_object_missing(&self, object_id: &ObjectId) -> bool {
        match self.missing_list.lock().unwrap().get(object_id) {
            Some(_) => true,
            None => false,
        }
    }

    pub fn miss_object(&self, object_id: &ObjectId) {
        match self.missing_list.lock().unwrap().entry(object_id.to_owned()) {
            Entry::Occupied(mut o) => {
                warn!("sync missing object but already exists! {}, last_sync={}", object_id, o.get().last_sync);
                o.get_mut().last_sync = bucky_time_now();
            }
            Entry::Vacant(v) => {
                warn!("sync missing object: {}", object_id);
                let state = ObjectState {
                    last_sync: bucky_time_now(),
                };

                v.insert(state);
            }
        }
    }

    pub fn filter_missing(&self, list: Vec<ObjectId>) -> Vec<ObjectId> {
        let missing_list = self.missing_list.lock().unwrap();
        list.into_iter().filter(|id| {
            missing_list.get(id).is_none()
        }).collect()
    }
}
