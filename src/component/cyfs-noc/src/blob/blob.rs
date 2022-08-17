use cyfs_base::*;
use cyfs_lib::*;

#[derive(Debug, Clone)]
pub struct BlobStorageStat {
    pub count: u64,
    pub storage_size: u64,
}

#[async_trait::async_trait]
pub trait BlobStorage: Send + Sync {
    async fn put_object(&self, data: NONObjectInfo) -> BuckyResult<()>;
    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<NONObjectInfo>>;
    async fn delete_object(&self, object_id: &ObjectId) -> BuckyResult<Option<NONObjectInfo>>;
    async fn exists_object(&self, object_id: &ObjectId) -> BuckyResult<bool>;
    async fn stat(&self) -> BuckyResult<BlobStorageStat>;
}
