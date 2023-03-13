use crate::archive::*;
use cyfs_base::*;

use async_std::sync::Arc;

#[async_trait::async_trait]
pub trait BackupDataLoader: Send + Sync {
    async fn verify(&self) -> BuckyResult<ObjectArchiveVerifyResult>;

    async fn meta(&self) -> BuckyResult<String>;

    // serialize methods
    async fn reset_object(&self);
    async fn next_object(&self) -> BuckyResult<Option<(ObjectId, ObjectArchiveInnerFile)>>;

    async fn reset_chunk(&self);
    async fn next_chunk(&self) -> BuckyResult<Option<(ObjectId, ObjectArchiveInnerFile)>>;

    // random methods
    async fn get_object(&self, object_id: &ObjectId)
        -> BuckyResult<Option<ObjectArchiveInnerFile>>;

    async fn get_chunk(&self, chunk_id: &ObjectId) -> BuckyResult<Option<ObjectArchiveInnerFile>>;
}

pub type BackupDataLoaderRef = Arc<Box<dyn BackupDataLoader>>;
