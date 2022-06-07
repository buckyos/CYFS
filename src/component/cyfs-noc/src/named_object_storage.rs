use async_trait::async_trait;
use cyfs_base::{BuckyResult, ObjectId};
use cyfs_lib::*;
use cyfs_util::*;

#[derive(Debug, Clone)]
pub enum NamedObjectStorageType {
    #[cfg(feature = "memory")]
    Memory = 0,

    #[cfg(feature = "mongo")]
    MongoDB = 1,

    #[cfg(feature = "sqlite")]
    Sqlite = 2,
}

impl Default for NamedObjectStorageType {
    fn default() -> Self {
        #[cfg(all(feature = "sqlite", not(feature = "mongo")))]
        let noc_type = NamedObjectStorageType::Sqlite;

        #[cfg(all(feature = "mongo", not(feature = "sqlite")))]
        let noc_type = NamedObjectStorageType::MongoDB;

        // 如果同时开启了，那么优先使用mongo
        #[cfg(all(feature = "mongo", feature = "sqlite"))]
        let noc_type = NamedObjectStorageType::MongoDB;

        noc_type
    }
}

impl std::fmt::Display for NamedObjectStorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match *self {
            #[cfg(feature = "memory")]
            Self::Memory => "memory",

            #[cfg(feature = "mongo")]
            Self::MongoDB => "mongodb",

            #[cfg(feature = "sqlite")]
            Self::Sqlite => "sqlite",
        };

        write!(f, "{}", msg)
    }
}

// insert object事件
pub type FnInsertObject = dyn EventListenerAsyncRoutine<ObjectCacheData, ()>;
pub type InsertObjectEventManager = SyncEventManagerSync<ObjectCacheData, ()>;

#[async_trait]
pub trait NamedObjectStorageEvent: Sync + Send + 'static {
    async fn pre_put(&self, object_info: &ObjectCacheData, first_time: bool) -> BuckyResult<()>;
    async fn post_put(&self, object_info: &ObjectCacheData, first_time: bool) -> BuckyResult<()>;
}

#[async_trait]
pub trait NamedObjectStorage: Sync + Send {
    async fn insert_object(
        &self,
        object_info: &ObjectCacheData,
        event: Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<NamedObjectCacheInsertResponse>;

    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>>;

    async fn select_object(
        &self,
        filter: &NamedObjectCacheSelectObjectFilter,
        opt: Option<&NamedObjectCacheSelectObjectOption>,
    ) -> BuckyResult<Vec<ObjectCacheData>>;

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult>;

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat>;

    fn clone(&self) -> Box<dyn NamedObjectStorage>;

    fn sync_server(&self) -> Option<Box<dyn NamedObjectCacheSyncServer>>;
    fn sync_client(&self) -> Option<Box<dyn NamedObjectCacheSyncClient>>;
}
