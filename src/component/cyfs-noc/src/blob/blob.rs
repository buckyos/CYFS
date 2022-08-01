use cyfs_lib::*;
use cyfs_base::*;

use std::path::PathBuf;

#[async_trait::async_trait]
pub trait BlobStorage {
    async fn put_object(&self, data: NONObjectInfo) -> BuckyResult<()>;
    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<NONObjectInfo>>;
    async fn delete_object(&self, object_id: &ObjectId) -> BuckyResult<bool>;
}
