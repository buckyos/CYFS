use super::log::BackupLogManager;
use crate::archive::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::AsyncReadWithSeek;

use async_std::sync::Arc;

#[async_trait::async_trait]
pub trait BackupDataWriter: Send + Sync {
    async fn add_isolate_meta(&self, isolate_meta: ObjectArchiveIsolateMeta);

    async fn add_object(
        &self,
        object_id: &ObjectId,
        object_raw: &[u8],
        meta: Option<&NamedObjectMetaData>,
    ) -> BuckyResult<()>;

    async fn add_chunk(
        &self,
        chunk_id: ChunkId,
        data: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<()>;

    fn logger(&self) -> Option<&BackupLogManager>;

    async fn finish(&self) -> BuckyResult<ObjectArchiveMeta>;
}

pub type BackupDataWriterRef = Arc<Box<dyn BackupDataWriter>>;
