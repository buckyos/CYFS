use crate::named_object_storage::*;

use async_trait::async_trait;
use cyfs_base::BuckyResult;
use cyfs_base::ObjectId;
use cyfs_debug::Mutex;

use lru_time_cache::{Entry, LruCache};
use std::sync::Arc;
use std::time::Duration;

pub(super) struct ObjectMemCacheInner {
    list: LruCache<ObjectId, ObjectCacheData>,
}

impl ObjectMemCacheInner {
    pub fn new(_isolate: &str) -> Self {
        Self {
            list: LruCache::with_expiry_duration(Duration::from_secs(60 * 10)),
        }
    }

    pub fn add_object(&mut self, obj_info: &ObjectCacheData) {
        assert!(obj_info.object.is_some());
        assert!(obj_info.object_raw.is_some());

        match self.list.entry(obj_info.object_id.clone()) {
            Entry::Occupied(v) => {
                info!("will replace old object: {}", obj_info.object_id);
                *v.into_mut() = obj_info.clone();
            }
            Entry::Vacant(v) => {
                info!("will save object: {}", obj_info.object_id);
                v.insert(obj_info.clone());
            }
        }
    }

    pub fn get_object(&mut self, object_id: &ObjectId) -> Option<ObjectCacheData> {
        let info = self.list.get(object_id);
        if info.is_none() {
            return None;
        }

        let obj = info.unwrap().clone();
        Some(obj)
    }
}

#[derive(Clone)]
pub(crate) struct ObjectMemCache(Arc<Mutex<ObjectMemCacheInner>>);

impl ObjectMemCache {
    pub fn new(isolate: &str, insert_object_event: InsertObjectEventManager) -> Self {
        let inner = ObjectMemCacheInner::new();
        let inner = Arc::new(Mutex::new(inner));

        Self(inner)
    }

    pub fn add_object(&self, obj_info: &ObjectCacheData) {
        self.0.lock().unwrap().add_object(obj_info)
    }

    pub fn get_object(&self, object_id: &ObjectId) -> Option<ObjectCacheData> {
        self.0.lock().unwrap().get_object(object_id)
    }
}

#[async_trait]
impl NamedObjectStorage for ObjectMemCache {
    async fn insert_object(
        &self,
        obj_info: &ObjectCacheData,
        _event: Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<()> {
        self.add_object(obj_info);
        Ok(())
    }

    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>> {
        Ok(self.get_object(object_id))
    }

    async fn select_object(
        &self,
        _filter: &NamedObjectCacheSelectObjectFilter,
        _opt: Option<&NamedObjectCacheSelectObjectOption>,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        unimplemented!();
    }

    fn sync_server(&self) -> Option<Box<dyn NamedObjectCacheSyncServer>> {
        unimplemented!();
    }

    fn sync_client(&self) -> Option<Box<dyn NamedObjectCacheSyncClient>> {
        unimplemented!();
    }
    fn clone(&self) -> Box<dyn NamedObjectStorage> {
        Box::new(Clone::clone(&self as &ObjectMemCache)) as Box<dyn NamedObjectStorage>
    }
}
