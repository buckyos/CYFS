use crate::named_object_storage::*;
use lazy_static::lazy_static;

#[cfg(feature = "mongo")]
use crate::mongodb::ObjectDBCache;

#[cfg(feature = "sqlite")]
use crate::sqlite::SqliteDBCache;

#[cfg(feature = "memory")]
use crate::memory::ObjectMemCache;

use cyfs_base::{BuckyResult, DeviceId, ObjectId};
use cyfs_lib::*;

use async_trait::async_trait;

pub struct ObjectCacheManager {
    device_id: DeviceId,
    cache: Option<Box<dyn NamedObjectStorage>>,

    insert_object_event: InsertObjectEventManager,
}

impl Clone for ObjectCacheManager {
    fn clone(&self) -> Self {
        let cache = self.cache.as_ref().unwrap();
        let cache = (*cache).clone();

        Self {
            device_id: self.device_id.clone(),
            cache: Some(cache),
            insert_object_event: self.insert_object_event.clone(),
        }
    }
}

impl ObjectCacheManager {
    pub fn new(device_id: &DeviceId) -> Self {
        Self {
            device_id: device_id.clone(),
            cache: None,
            insert_object_event: InsertObjectEventManager::new(),
        }
    }

    pub async fn init(
        &mut self,
        cache_type: NamedObjectStorageType,
        isolate: &str,
    ) -> BuckyResult<()> {
        assert!(self.cache.is_none());
        let cache = self.create_cache(&cache_type, isolate).await?;

        self.cache = Some(cache);

        Ok(())
    }

    pub fn insert_object_event(&self) -> &InsertObjectEventManager {
        &self.insert_object_event
    }

    async fn create_cache(
        &self,
        cache_type: &NamedObjectStorageType,
        isolate: &str,
    ) -> BuckyResult<Box<dyn NamedObjectStorage>> {
        let ret = match cache_type {
            #[cfg(feature = "memory")]
            NamedObjectStorageType::Memory => Box::new(ObjectMemCache::new(
                isolate,
                self.insert_object_event.clone(),
            )) as Box<dyn NamedObjectStorage>,

            #[cfg(feature = "mongo")]
            NamedObjectStorageType::MongoDB => {
                Box::new(ObjectDBCache::new(isolate, self.insert_object_event.clone()).await?)
                    as Box<dyn NamedObjectStorage>
            }

            #[cfg(feature = "sqlite")]
            NamedObjectStorageType::Sqlite => Box::new(SqliteDBCache::new(
                isolate,
                self.insert_object_event.clone(),
            )?) as Box<dyn NamedObjectStorage>,
        };

        Ok(ret)
    }

    // debug???????????????????????????
    #[cfg(debug_assertions)]
    async fn insert_object(
        &self,
        obj_info: &NamedObjectCacheInsertObjectRequest,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        let this = self.clone();
        let obj_info = obj_info.to_owned();
        async_std::task::spawn(async move { this.insert_object_with_event(&obj_info, None).await })
            .await
    }

    #[cfg(not(debug_assertions))]
    async fn insert_object(
        &self,
        obj_info: &NamedObjectCacheInsertObjectRequest,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        self.insert_object_with_event(obj_info, None).await
    }

    pub async fn insert_object_with_event(
        &self,
        obj_info: &NamedObjectCacheInsertObjectRequest,
        event: Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        let mut data = ObjectCacheData::from(obj_info.clone());

        // ?????????????????????device_id????????????????????????????????????
        if data.source == DeviceId::default() {
            data.source = self.device_id.clone();
        }

        data.rebuild_object()?;

        data.update_insert_time();

        let ret = self
            .cache
            .as_ref()
            .unwrap()
            .insert_object(&data, event)
            .await;

        /*
        ??????????????????????????????storage??????????????????post_put????????????????????????????????????????????????
        if ret.is_ok() {
            // ???????????????????????????????????????
            let _ = self.insert_object_event.emit(&data);
        }
        */
        ret
    }

    // debug???????????????????????????
    #[cfg(debug_assertions)]
    async fn direct_get_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectCacheData>> {
        let cache = (*self.cache.as_ref().unwrap()).clone();
        let object_id = object_id.to_owned();
        async_std::task::spawn(async move { cache.get_object(&object_id).await }).await
    }

    #[cfg(not(debug_assertions))]
    async fn direct_get_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectCacheData>> {
        self.cache.as_ref().unwrap().get_object(object_id).await
    }

    async fn select_object(
        &self,
        req: &NamedObjectCacheSelectObjectRequest,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        self.cache
            .as_ref()
            .unwrap()
            .select_object(&req.filter, req.opt.as_ref())
            .await
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult> {
        self.cache.as_ref().unwrap().delete_object(req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        self.cache.as_ref().unwrap().stat().await
    }
}

#[async_trait]
impl NamedObjectCache for ObjectCacheManager {
    async fn insert_object(
        &self,
        obj_info: &NamedObjectCacheInsertObjectRequest,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        ObjectCacheManager::insert_object(&self, obj_info).await
    }

    async fn get_object(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<ObjectCacheData>> {
        ObjectCacheManager::direct_get_object(&self, &req.object_id).await
    }

    async fn select_object(
        &self,
        req: &NamedObjectCacheSelectObjectRequest,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        ObjectCacheManager::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult> {
        ObjectCacheManager::delete_object(&self, req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        ObjectCacheManager::stat(&self).await
    }

    fn sync_server(&self) -> Option<Box<dyn NamedObjectCacheSyncServer>> {
        self.cache.as_ref().unwrap().sync_server()
    }

    fn sync_client(&self) -> Option<Box<dyn NamedObjectCacheSyncClient>> {
        self.cache.as_ref().unwrap().sync_client()
    }

    fn clone_noc(&self) -> Box<dyn NamedObjectCache> {
        Box::new(Clone::clone(&self as &ObjectCacheManager)) as Box<dyn NamedObjectCache>
    }
}

lazy_static! {
    pub static ref OBJECT_CACHE_MANAGER: ObjectCacheManager =
        ObjectCacheManager::new(&DeviceId::default());
}
