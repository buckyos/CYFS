use super::local::ArchiveLocalFileWriter;
use super::log::BackupLogManager;
use super::writer::*;
use crate::archive::*;
use crate::meta::*;
use crate::object_pack::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::{AsyncReadWithSeek};

use async_std::sync::{Arc};
use std::path::PathBuf;

#[derive(Clone)]
pub struct StateBackupDataLocalFileWriter {
    archive: ArchiveLocalFileWriter,
    meta: ObjectArchiveStateMetaHolder,
    log: Arc<BackupLogManager>,
}

impl StateBackupDataLocalFileWriter {
    pub fn new(
        id: u64,
        default_isolate: ObjectId,
        root: PathBuf,
        format: ObjectPackFormat,
        archive_file_max_size: u64,
    ) -> BuckyResult<Self> {
        let log_dir = root.join("log");
        if !log_dir.is_dir() {
            std::fs::create_dir_all(&log_dir).map_err(|e| {
                let msg = format!("create backup log dir failed! {}, {}", log_dir.display(), e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
        }

        let log = BackupLogManager::new(default_isolate, log_dir);
        let meta = ObjectArchiveStateMetaHolder::new(id);

        let archive = ArchiveLocalFileWriter::new(id, root, format, archive_file_max_size)?;

        Ok(Self {
            archive,
            meta,
            log: Arc::new(log),
        })
    }

    pub fn into_writer(self) -> BackupDataWriterRef {
        Arc::new(Box::new(self))
    }

    pub async fn finish(&self) -> BuckyResult<(ObjectArchiveIndex, ObjectArchiveStateMeta)> {
        let index = self.archive.finish().await?;
        let meta = self.meta.finish();

        Ok((index, meta))
    }
}

#[async_trait::async_trait]
impl BackupDataWriter for StateBackupDataLocalFileWriter {
    async fn add_isolate_meta(&self, isolate_meta: ObjectArchiveIsolateMeta) {
        self.meta.add_isolate_meta(isolate_meta)
    }

    async fn add_object(
        &self,
        object_id: &ObjectId,
        object_raw: &[u8],
        meta: Option<&NamedObjectMetaData>,
    ) -> BuckyResult<()> {
        self.archive.add_object(object_id, object_raw, meta).await
    }

    async fn add_chunk(
        &self,
        chunk_id: ChunkId,
        data: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<()> {
        self.archive.add_chunk(chunk_id, data, meta).await
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
