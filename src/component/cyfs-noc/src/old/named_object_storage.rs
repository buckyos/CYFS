use async_trait::async_trait;
use cyfs_base::{BuckyResult, ObjectId};
use cyfs_lib::*;
use cyfs_util::*;

#[derive(Debug, Clone)]
pub enum NamedObjectStorageType {
    MongoDB = 1,
    Sqlite = 2,
}

impl Default for NamedObjectStorageType {
    fn default() -> Self {
        let noc_type = NamedObjectStorageType::Sqlite;

        noc_type
    }
}

impl std::fmt::Display for NamedObjectStorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match *self {
            Self::MongoDB => "mongodb",
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
