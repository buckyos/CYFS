use super::chunk_writer::ChunkWriter;
use super::state::*;
use cyfs_base::*;
use cyfs_chunk_cache::LocalFile;
use cyfs_chunk_lib::{Chunk, ChunkRead};
use cyfs_util::cache::{NamedDataCache, TrackerCache};

use cyfs_debug::Mutex;
use futures::AsyncWriteExt;
use std::path::PathBuf;
use std::sync::Arc;

pub struct LocalFileWriter {
    file_path: PathBuf,
    file_id: FileId,
    local_file: Arc<async_std::sync::Mutex<LocalFile>>,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    err: Arc<Mutex<Option<BuckyError>>>,
    state: Arc<LocalFileStateUpdater>,
}

impl LocalFileWriter {
    pub async fn new(
        path: PathBuf,
        file: File,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
    ) -> BuckyResult<Self> {
        Ok(Self {
            file_path: path.clone(),
            file_id: file.desc().file_id(),
            local_file: Arc::new(async_std::sync::Mutex::new(
                LocalFile::open(path.clone(), file.clone()).await?,
            )),
            ndc,
            tracker,
            err: Arc::new(Mutex::new(None)),
            state: Arc::new(LocalFileStateUpdater::new(file, path)),
        })
    }
}

#[async_trait::async_trait]
impl ChunkWriter for LocalFileWriter {
    async fn write(&self, chunk_id: &ChunkId, chunk: Box<dyn Chunk>) -> BuckyResult<()> {
        info!(
            "will write chunk to local file! file={}, chunk={}, path={}",
            self.file_id,
            chunk_id,
            self.file_path.display()
        );

        {
            let mut local_file = self.local_file.lock().await;
            local_file.put_chunk(chunk_id, chunk.as_ref()).await?;
        }

        self.state.update_chunk_state(&self.ndc, chunk_id).await?;
        self.state
            .update_chunk_tracker(&self.tracker, chunk_id)
            .await?;

        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        let local_file = self.local_file.lock().await;
        local_file.flush().await.map_err(|e| {
            error!("flush file failed! file={}, path={}, {}", self.file_id, self.file_path.display(), e);
            e
        })?;

        Ok(())
    }

    async fn err(&self, e: &BuckyError) -> BuckyResult<()> {
        error!(
            "local file write failed! file={}, path={}, {}",
            self.file_id,
            self.file_path.display(),
            e
        );

        *self.err.lock().unwrap() = Some(e.to_owned());
        Ok(())
    }
}

pub struct LocalChunkWriter {
    local_path: PathBuf,
    chunk_id: ChunkId,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    err: Arc<Mutex<Option<BuckyError>>>,
    state: Arc<LocalChunkStateUpdater>,
}

impl LocalChunkWriter {
    pub fn new(
        local_path: PathBuf,
        chunk_id: ChunkId,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
    ) -> Self {
        Self {
            local_path: local_path.clone(),
            chunk_id,
            ndc,
            tracker,
            err: Arc::new(Mutex::new(None)),
            state: Arc::new(LocalChunkStateUpdater::new(local_path)),
        }
    }
}

#[async_trait::async_trait]
impl ChunkWriter for LocalChunkWriter {
    async fn write(&self, chunk_id: &ChunkId, chunk: Box<dyn Chunk>) -> BuckyResult<()> {
        info!(
            "will write chunk to local file! chunk={}, path={}",
            chunk_id,
            self.local_path.display()
        );

        let reader = ChunkRead::new(chunk);

        let mut file = async_std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .open(self.local_path.as_path())
            .await
            .map_err(|e| {
                let msg = format!(
                    "write chunk but create file failed! chunk={}, file={}, {}",
                    self.chunk_id,
                    self.local_path.display(),
                    e
                );
                log::error!("{}", msg.as_str());
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        async_std::io::copy(reader, file.clone())
            .await
            .map_err(|e| {
                let msg = format!(
                    "write chunk to file failed! chunk={}, len={}, file={}, {}",
                    self.chunk_id,
                    self.chunk_id.len(),
                    self.local_path.display(),
                    e
                );
                log::error!("{}", msg.as_str());
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        file.flush().await.map_err(|e| {
            let msg = format!(
                "write chunk to file but flush failed! chunk={}, file={}, {}",
                self.chunk_id,
                self.local_path.display(),
                e
            );
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        self.state.update_chunk_state(&self.ndc, chunk_id).await?;
        self.state
            .update_chunk_tracker(&self.tracker, chunk_id)
            .await?;

        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        Ok(())
    }

    async fn err(&self, e: &BuckyError) -> BuckyResult<()> {
        error!(
            "local chunk file write failed! chunk={}, file={}, {}",
            self.chunk_id,
            self.local_path.display(),
            e
        );

        *self.err.lock().unwrap() = Some(e.to_owned());
        Ok(())
    }
}
