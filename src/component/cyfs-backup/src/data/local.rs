use super::log::BackupLogManager;
use crate::archive::*;
use crate::object_pack::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::{AsyncReadWithSeek, AsyncReadWithSeekAdapter};
use super::writer::*;

use async_std::sync::{Arc, Mutex as AsyncMutex};
use std::ops::DerefMut;
use std::path::PathBuf;

#[derive(Clone)]
pub struct BackupDataLocalFileWriter {
    archive: Arc<AsyncMutex<ObjectArchiveGenerator>>,
    log: Arc<BackupLogManager>,
}

impl BackupDataLocalFileWriter {
    pub fn new(
        id: u64,
        default_isolate: ObjectId,
        root: PathBuf,
        format: ObjectPackFormat,
        archive_file_max_size: u64,
    ) -> BuckyResult<Self> {
        let data_dir = root.join("data");
        if !data_dir.is_dir() {
            std::fs::create_dir_all(&data_dir).map_err(|e| {
                let msg = format!(
                    "create backup data dir failed! {}, {}",
                    data_dir.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
        }

        let log_dir = root.join("log");
        if !log_dir.is_dir() {
            std::fs::create_dir_all(&log_dir).map_err(|e| {
                let msg = format!("create backup log dir failed! {}, {}", log_dir.display(), e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
        }

        let archive = ObjectArchiveGenerator::new(
            id,
            format,
            data_dir,
            archive_file_max_size,
        );
        let log = BackupLogManager::new(default_isolate, log_dir);

        Ok(Self {
            archive: Arc::new(AsyncMutex::new(archive)),
            log: Arc::new(log),
        })
    }

    pub fn into_writer(self) -> BackupDataWriterRef {
        Arc::new(Box::new(self))
    }

    pub async fn add_isolate_meta(&self, isolate_meta: ObjectArchiveIsolateMeta) {
        let mut archive = self.archive.lock().await;
        archive.add_isolate_meta(isolate_meta);
    }

    pub async fn add_object(
        &self,
        object_id: &ObjectId,
        object_raw: &[u8],
        meta: Option<&NamedObjectMetaData>,
    ) -> BuckyResult<()> {
        let meta = meta.map(|item| item.into());

        let mut archive = self.archive.lock().await;
        archive.add_data_buf(object_id, object_raw, meta).await?;

        Ok(())
    }

    pub async fn add_chunk(
        &self,
        chunk_id: ChunkId,
        data: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<()> {
        let reader = AsyncReadWithSeekAdapter::new(data).into_reader();
        let mut archive = self.archive.lock().await;
        archive.add_data(chunk_id.as_object_id(), reader, meta).await?;

        Ok(())
    }

    pub async fn finish(&self) -> BuckyResult<ObjectArchiveMeta> {
        let archive = {
            let mut archive = self.archive.lock().await;
            let mut empty_archive = archive.clone_empty();
            std::mem::swap(archive.deref_mut(), &mut empty_archive);

            empty_archive
        };

        /*
        let archive = match Arc::try_unwrap(self.archive) {
            Ok(ret) => ret,
            Err(_) => unreachable!(),
        };

        let archive = archive.into_inner();
        */

        archive.finish().await
    }
}


#[async_trait::async_trait]
impl BackupDataWriter for BackupDataLocalFileWriter {
    async fn add_isolate_meta(&self, isolate_meta: ObjectArchiveIsolateMeta) {
        Self::add_isolate_meta(&self, isolate_meta).await
    }

    async fn add_object(
        &self,
        object_id: &ObjectId,
        object_raw: &[u8],
        meta: Option<&NamedObjectMetaData>,
    ) -> BuckyResult<()> {
        Self::add_object(&self, object_id, object_raw, meta).await
    }

    async fn add_chunk(
        &self,
        chunk_id: ChunkId,
        data: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<()> {
        Self::add_chunk(&self, chunk_id, data, meta).await
    }

    async fn on_error(
        &self,
        isolate_id: Option<&ObjectId>,
        dec_id: Option<&ObjectId>,
        id: &ObjectId,
        e: BuckyError,
    ) -> BuckyResult<()> {
        self.log.on_error(isolate_id, dec_id, id, e);

        Ok(())
    }

    async fn on_missing(
        &self,
        isolate_id: Option<&ObjectId>,
        dec_id: Option<&ObjectId>,
        id: &ObjectId,
    ) -> BuckyResult<()> {
        self.log.on_missing(isolate_id, dec_id, id);

        Ok(())
    }
}