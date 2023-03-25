use crate::restore::StackLocalObjectComponents;
use cyfs_base::*;
use cyfs_bdt::ChunkReaderRef;
use cyfs_lib::*;
use cyfs_noc::*;
use cyfs_util::AsyncReadWithSeek;

use std::path::Path;
use std::sync::Arc;

pub struct UniBackupObjectLoader {
    object_storage: Box<dyn BlobStorage>,
    chunk_store: ChunkReaderRef,
}

impl UniBackupObjectLoader {
    pub async fn create(
        cyfs_root: &Path,
        isolate: &str,
        chunk_store: ChunkReaderRef,
    ) -> BuckyResult<Self> {
        let object_storage =
            StackLocalObjectComponents::create_object_storage(cyfs_root, isolate).await?;

        Ok(Self {
            object_storage,
            chunk_store,
        })
    }

    pub fn into_reader(self) -> ObjectTraverserLoaderRef {
        Arc::new(Box::new(self))
    }
}

#[async_trait::async_trait]
impl ObjectTraverserLoader for UniBackupObjectLoader {
    async fn get_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectTraverserLoaderObjectData>> {
        self.object_storage
            .get_object(object_id)
            .await
            .map(|data| data.map(|object| ObjectTraverserLoaderObjectData { object, meta: None }))
    }

    async fn get_chunk(
        &self,
        chunk_id: &ChunkId,
    ) -> BuckyResult<Option<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>>> {
        match self.chunk_store.get(chunk_id).await {
            Ok(chunk) => Ok(Some(chunk)),
            Err(e) if e.code() == BuckyErrorCode::NotFound => {
                warn!(
                    "traverser get chunk from chunk store but not found! {}",
                    chunk_id
                );
                Ok(None)
            }
            Err(e) => {
                error!(
                    "traverser get chunk from chunk store error! {}, {}",
                    chunk_id, e
                );
                Err(e)
            }
        }
    }
}
