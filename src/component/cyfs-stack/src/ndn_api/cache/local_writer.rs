use super::state::*;
use cyfs_base::*;
use super::chunk_writer::ChunkWriter;
use cyfs_chunk_cache::{LocalFile};
use cyfs_util::cache::{NamedDataCache, TrackerCache};
use cyfs_chunk_lib::{Chunk, ChunkRead};

use futures::AsyncWriteExt;
use std::path::PathBuf;
use std::sync::Arc;
use cyfs_debug::Mutex;

pub struct LocalFileWriter {
    file_path: PathBuf,
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
        info!("will write chunk to local file! chunk={}, file={}", chunk_id, self.file_path.display());
        
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
        local_file.flush().await?;
        Ok(())
    }

    async fn err(&self, e: &BuckyError) -> BuckyResult<()> {
        error!("local file write failed! file={}, {}", self.file_path.display(), e);

        *self.err.lock().unwrap() = Some(e.to_owned());
        Ok(())
    }
}

pub struct LocalChunkWriter {
    local_path: PathBuf,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    err: Arc<Mutex<Option<BuckyError>>>,
    state: Arc<LocalChunkStateUpdater>,
}

impl LocalChunkWriter {
    pub fn new(
        local_path: PathBuf,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
    ) -> Self {
        Self {
            local_path: local_path.clone(),
            ndc,
            tracker,
            err: Arc::new(Mutex::new(None)),
            state: Arc::new(LocalChunkStateUpdater::new(local_path)),
        }
    }
}

impl std::fmt::Display for LocalChunkWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "write chunk {}",
            self.local_path.to_string_lossy().to_string()
        )
    }
}

#[async_trait::async_trait]
impl ChunkWriter for LocalChunkWriter {
    async fn write(&self, chunk_id: &ChunkId, chunk: Box<dyn Chunk>) -> BuckyResult<()> {
        let reader = ChunkRead::new(chunk);

        let mut file = async_std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .open(self.local_path.as_path())
            .await
            .map_err(|e| {
                let msg = format!(
                    "[{}:{}] open {} failed.err {}",
                    file!(),
                    line!(),
                    self.local_path.to_string_lossy().to_string(),
                    e
                );
                log::error!("{}", msg.as_str());
                BuckyError::new(BuckyErrorCode::Failed, msg)
            })?;

        async_std::io::copy(reader, file.clone()).await.map_err(|e| {
            let msg = format!(
                "[{}:{}] write {} failed.err {}",
                file!(),
                line!(),
                self.local_path.to_string_lossy().to_string(),
                e
            );
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;

        file.flush().await.map_err(|e| {
            let msg = format!(
                "[{}:{}] flush {} failed.err {}",
                file!(),
                line!(),
                self.local_path.to_string_lossy().to_string(),
                e
            );
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
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
        error!("local chunk file write failed! file={}, {}", self.local_path.display(), e);

        *self.err.lock().unwrap() = Some(e.to_owned());
        Ok(())
    }
}
