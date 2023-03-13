use crate::archive::*;
use crate::data::*;
use crate::meta::*;
use crate::object_pack::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::AsyncReadWithSeek;

use async_std::sync::Arc;
use std::path::PathBuf;

#[derive(Clone)]
pub struct StateBackupDataLocalFileWriter {
    archive: ArchiveLocalFileWriter,
    loader: ObjectTraverserLoaderRef,
    meta: ObjectArchiveStateMetaHolder,
    log: Arc<BackupLogManager>,
}

impl StateBackupDataLocalFileWriter {
    pub fn new(
        id: u64,
        state_default_isolate: ObjectId,
        root: PathBuf,
        format: ObjectPackFormat,
        archive_file_max_size: u64,
        loader: ObjectTraverserLoaderRef,
    ) -> BuckyResult<Self> {
        let log_dir = root.join("log");
        if !log_dir.is_dir() {
            std::fs::create_dir_all(&log_dir).map_err(|e| {
                let msg = format!("create backup log dir failed! {}, {}", log_dir.display(), e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
        }

        let log = BackupLogManager::new(Some(state_default_isolate), log_dir);
        let meta = ObjectArchiveStateMetaHolder::new();

        let archive = ArchiveLocalFileWriter::new(id, root, format, ObjectBackupStrategy::State, archive_file_max_size)?;

        Ok(Self {
            archive,
            meta,
            log: Arc::new(log),
            loader,
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
        self.archive.add_object(object_id, object_raw, meta).await?;

        Ok(())
    }

    async fn add_chunk(
        &self,
        isolate_id: Option<&ObjectId>,
        dec_id: Option<&ObjectId>,
        chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        match self.loader.get_chunk(chunk_id).await {
            Ok(Some(data)) => {
                match self
                    .archive
                    .add_chunk(chunk_id.to_owned(), data, None)
                    .await?
                {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        self.on_error(isolate_id, dec_id, chunk_id.as_object_id(), e)
                            .await
                    }
                }
            }
            Ok(None) => {
                self.on_missing(isolate_id, dec_id, chunk_id.as_object_id())
                    .await
            }
            Err(e) => {
                self.on_error(isolate_id, dec_id, chunk_id.as_object_id(), e)
                    .await
            }
        }
    }

    async fn add_chunk_data(
        &self,
        isolate_id: Option<&ObjectId>,
        dec_id: Option<&ObjectId>,
        chunk_id: &ChunkId,
        data: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<()> {
        match self
            .archive
            .add_chunk(chunk_id.to_owned(), data, meta)
            .await?
        {
            Ok(_) => Ok(()),
            Err(e) => {
                self.on_error(isolate_id, dec_id, chunk_id.as_object_id(), e)
                    .await
            }
        }
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
