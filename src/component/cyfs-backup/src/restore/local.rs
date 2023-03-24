use super::restorer::*;
use crate::archive::*;
use cyfs_base::*;
use cyfs_chunk_cache::ChunkCache;
use cyfs_lib::NONObjectInfo;
use cyfs_noc::BlobStorage;

use async_std::io::WriteExt;
use std::path::{Path, PathBuf};

pub struct StackLocalObjectRestorer {
    cyfs_root: PathBuf,
    isolate: String,

    object_storage: Box<dyn BlobStorage>,
    chunk_storage: Box<dyn ChunkCache>,
}

impl StackLocalObjectRestorer {
    pub async fn create(cyfs_root: PathBuf, isolate: &str) -> BuckyResult<Self> {
        let dir = cyfs_util::get_cyfs_root_path().join("data");
        let dir = if isolate.len() > 0 {
            dir.join(isolate)
        } else {
            dir
        };
        let noc_dir = dir.join("named-object-cache");

        let object_storage = cyfs_noc::create_blob_storage(&noc_dir).await?;

        let chunk_storage = cyfs_chunk_cache::create_local_chunk_cache(&dir, isolate).await?;

        let ret = Self {
            cyfs_root,
            isolate: isolate.into(),
            object_storage,
            chunk_storage,
        };

        Ok(ret)
    }

    async fn restore_file(
        &self,
        inner_path: &Path,
        data: ObjectArchiveInnerFileData,
    ) -> BuckyResult<()> {
        if inner_path.is_absolute() {
            let msg = format!(
                "invalid restore file's inner path! {}",
                inner_path.display()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let full_path = self.cyfs_root.join(inner_path);
        if let Some(parent) = full_path.parent() {
            if !parent.is_dir() {
                async_std::fs::create_dir_all(&parent).await.map_err(|e| {
                    let msg = format!(
                        "create restore file's dir error: {}, err={}",
                        parent.display(),
                        e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;
            }
        }

        let mut opt = async_std::fs::OpenOptions::new();
        opt.write(true).create(true).truncate(true);

        let mut outfile = opt.open(&full_path).await.map_err(|e| {
            let msg = format!("create file error: {}, err={}", full_path.display(), e);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let s = data.into_stream();
        async_std::io::copy(s, outfile.clone()).await.map_err(|e| {
            let msg = format!(
                "write data to restore file error: {}, err={}",
                full_path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        outfile.flush().await.map_err(|e| {
            let msg = format!(
                "flush data to restore file error: {}, err={}",
                full_path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!("restore local file complete! file={}", full_path.display());
        Ok(())
    }

    async fn restore_object(
        &self,
        object_id: &ObjectId,
        data: ObjectArchiveInnerFile,
    ) -> BuckyResult<()> {
        let object_raw = data.data.into_buffer().await.map_err(|e| {
            let msg = format!("restore object failed! id={}, {}", object_id, e);
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        let info = NONObjectInfo::new(object_id.to_owned(), object_raw, None);
        self.object_storage.put_object(info).await.map_err(|e| {
            let msg = format!(
                "restore object to object blob cache failed! id={}, {}",
                object_id, e
            );
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        Ok(())
    }

    async fn restore_chunk(
        &self,
        chunk_id: &ChunkId,
        data: ObjectArchiveInnerFile,
    ) -> BuckyResult<()> {
        let buf = data.data.into_buffer().await.map_err(|e| {
            let msg = format!("restore chunk failed! id={}, {}", chunk_id, e);
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        let chunk = cyfs_chunk_lib::ChunkMeta::MemChunk(buf)
            .to_chunk()
            .await
            .map_err(|e| {
                let msg = format!(
                    "create chunk buf to chunk object failed! id={}, {}",
                    chunk_id, e
                );
                error!("{}", msg);
                BuckyError::new(e.code(), msg)
            })?;

        self.chunk_storage
            .put_chunk(chunk_id, chunk)
            .await
            .map_err(|e| {
                let msg = format!(
                    "restore chunk to chunk cache failed! id={}, {}",
                    chunk_id, e
                );
                error!("{}", msg);
                BuckyError::new(e.code(), msg)
            })?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ObjectRestorer for StackLocalObjectRestorer {
    async fn restore_file(
        &self,
        inner_path: &Path,
        data: ObjectArchiveInnerFileData,
    ) -> BuckyResult<()> {
        Self::restore_file(&self, inner_path, data).await
    }

    async fn restore_object(
        &self,
        object_id: &ObjectId,
        data: ObjectArchiveInnerFile,
    ) -> BuckyResult<()> {
        Self::restore_object(&self, object_id, data).await
    }

    async fn restore_chunk(
        &self,
        chunk_id: &ChunkId,
        data: ObjectArchiveInnerFile,
    ) -> BuckyResult<()> {
        Self::restore_chunk(&self, chunk_id, data).await
    }
}
