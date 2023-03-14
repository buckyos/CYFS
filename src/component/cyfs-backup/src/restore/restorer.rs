use crate::archive::*;
use cyfs_base::*;

use std::path::Path;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ObjectRestorer: Send + Sync {
    async fn restore_file(
        &self,
        inner_path: &Path,
        data: ObjectArchiveInnerFileData,
    ) -> BuckyResult<()>;

    async fn restore_object(
        &self,
        object_id: &ObjectId,
        data: ObjectArchiveInnerFile,
    ) -> BuckyResult<()>;

    async fn restore_chunk(
        &self,
        chunk_id: &ChunkId,
        data: ObjectArchiveInnerFile,
    ) -> BuckyResult<()>;
}

pub type ObjectRestorerRef = Arc<Box<dyn ObjectRestorer>>;
