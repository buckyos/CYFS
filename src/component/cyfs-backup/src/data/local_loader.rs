use crate::{archive::*, crypto::ProtectedPassword};
use cyfs_base::*;
use super::loader::*;

use async_std::sync::{Arc, Mutex as AsyncMutex};
use std::path::PathBuf;

#[derive(Clone)]
pub struct ArchiveLocalFileLoader {
    archive_dir: PathBuf,
    archive: Arc<AsyncMutex<ObjectArchiveLoader>>,
}

impl ArchiveLocalFileLoader {
    pub async fn load(
        archive_dir: PathBuf,
        password: Option<ProtectedPassword>,
    ) -> BuckyResult<Self> {
        let archive = ObjectArchiveLoader::load(archive_dir.clone(), password).await?;

        Ok(Self {
            archive_dir,
            archive: Arc::new(AsyncMutex::new(archive)),
        })
    }

    async fn load_meta(&self) -> BuckyResult<serde_json::Value> {
        let index = self.index().await;
        if index.meta.is_none() {
            let msg = format!(
                "load meta info from index but not exists! index={:?}",
                index,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        Ok(index.meta.as_ref().unwrap().clone())
    }
}

#[async_trait::async_trait]
impl BackupDataLoader for ArchiveLocalFileLoader {
    async fn verify(&self) -> BuckyResult<ObjectArchiveVerifyResult> {
        let mut loader = self.archive.lock().await;
        loader.random_reader().verify().await
    }

    async fn index(&self) -> ObjectArchiveIndex {
        let mut loader = self.archive.lock().await;
        loader.random_reader().index().to_owned()
    }

    async fn meta(&self) -> BuckyResult<serde_json::Value> {
        Self::load_meta(&self).await
    }

    // serialize methods
    async fn reset_object(&self) {
        let mut loader = self.archive.lock().await;
        loader.serialize_reader().reset_object()
    }

    async fn next_object(&self) -> BuckyResult<Option<(ObjectId, ObjectArchiveInnerFile)>> {
        let mut loader = self.archive.lock().await;
        loader.serialize_reader().next_object().await
    }

    async fn reset_chunk(&self) {
        let mut loader = self.archive.lock().await;
        loader.serialize_reader().reset_chunk()
    }

    async fn next_chunk(&self) -> BuckyResult<Option<(ChunkId, ObjectArchiveInnerFile)>> {
        let mut loader = self.archive.lock().await;
        loader.serialize_reader().next_chunk().await
    }

    // random methods
    async fn get_object(&self, object_id: &ObjectId)
        -> BuckyResult<Option<ObjectArchiveInnerFile>> {
            let mut loader = self.archive.lock().await;
            loader.random_reader().get_object(object_id).await
        }

    async fn get_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<Option<ObjectArchiveInnerFile>> {
        let mut loader = self.archive.lock().await;
        loader.random_reader().get_chunk(chunk_id).await
    }
}