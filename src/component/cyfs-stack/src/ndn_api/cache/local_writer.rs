use super::state::*;
use cyfs_base::*;
use cyfs_bdt::ChunkWriter;
use cyfs_chunk_cache::{LocalFile, MemRefChunk};
use cyfs_util::cache::{NamedDataCache, TrackerCache};

use futures::AsyncWriteExt;
use std::path::PathBuf;
use std::sync::Arc;
use cyfs_debug::Mutex;

pub struct LocalFileWriter {
    file_path: PathBuf,
    local_file: Arc<async_std::sync::Mutex<LocalFile>>,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    err: Arc<Mutex<BuckyErrorCode>>,
    state: Arc<LocalFileStateUpdater>,
}

impl Clone for LocalFileWriter {
    fn clone(&self) -> Self {
        Self {
            file_path: self.file_path.clone(),
            local_file: self.local_file.clone(),
            ndc: self.ndc.clone(),
            tracker: self.tracker.clone(),
            err: self.err.clone(),
            state: self.state.clone(),
        }
    }
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
            err: Arc::new(Mutex::new(BuckyErrorCode::Ok)),
            state: Arc::new(LocalFileStateUpdater::new(file, path)),
        })
    }
}

impl std::fmt::Display for LocalFileWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "write file {}",
            self.file_path.to_string_lossy().to_string()
        )
    }
}

#[async_trait::async_trait]
impl ChunkWriter for LocalFileWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone())
    }

    async fn write(&self, chunk_id: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        let ref_chunk = MemRefChunk::from(unsafe {
            std::mem::transmute::<_, &'static [u8]>(content.as_slice())
        });

        {
            let mut local_file = self.local_file.lock().await;
            local_file.put_chunk(chunk_id, &ref_chunk).await?;
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

    async fn err(&self, e: BuckyErrorCode) -> BuckyResult<()> {
        *self.err.lock().unwrap() = e;
        Ok(())
    }
}

pub struct LocalChunkWriter {
    local_path: PathBuf,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    err: Arc<Mutex<BuckyErrorCode>>,
    state: Arc<LocalChunkStateUpdater>,
}

impl Clone for LocalChunkWriter {
    fn clone(&self) -> Self {
        Self {
            local_path: self.local_path.clone(),
            ndc: self.ndc.clone(),
            tracker: self.tracker.clone(),
            err: self.err.clone(),
            state: self.state.clone(),
        }
    }
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
            err: Arc::new(Mutex::new(BuckyErrorCode::Ok)),
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
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone())
    }

    async fn write(&self, chunk_id: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
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
        file.write(content.as_slice()).await.map_err(|e| {
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

    async fn err(&self, e: BuckyErrorCode) -> BuckyResult<()> {
        *self.err.lock().unwrap() = e;
        Ok(())
    }
}
