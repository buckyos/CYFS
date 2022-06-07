use cyfs_base::*;
use cyfs_bdt::ChunkWriter;

use cyfs_chunk_cache::{LocalFile, MemRefChunk};
use futures::{AsyncWriteExt};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use cyfs_util::cache::{
    AddTrackerPositonRequest, NamedDataCache, PostionFileRange, TrackerCache, TrackerDirection,
    TrackerPostion, UpdateChunkStateRequest,
};

pub struct LocalFileWriter {
    file_path: PathBuf,
    local_file: Arc<async_std::sync::Mutex<LocalFile>>,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    err: Arc<Mutex<BuckyErrorCode>>,
}

impl Clone for LocalFileWriter {
    fn clone(&self) -> Self {
        Self {
            file_path: self.file_path.clone(),
            local_file: self.local_file.clone(),
            ndc: self.ndc.clone(),
            tracker: self.tracker.clone(),
            err: self.err.clone(),
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
                LocalFile::open(path, file).await?,
            )),
            ndc,
            tracker,
            err: Arc::new(Mutex::new(BuckyErrorCode::Ok)),
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

    async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        let ref_chunk = MemRefChunk::from(unsafe {
            std::mem::transmute::<_, &'static [u8]>(content.as_slice())
        });

        let chunk_range_list = {
            let mut local_file = self.local_file.lock().await;
            local_file.put_chunk(chunk, &ref_chunk).await?;

            local_file.get_chunk_range_list(chunk).await?.clone()
        };

        self.ndc
            .update_chunk_state(&UpdateChunkStateRequest {
                chunk_id: chunk.clone(),
                current_state: None,
                state: ChunkState::Ready,
            })
            .await
            .map_err(|e| {
                error!("{} add to tracker failed for {}", self, e);
                e
            })?;

        for (offset, length) in chunk_range_list.iter() {
            let request = AddTrackerPositonRequest {
                id: chunk.to_string(),
                direction: TrackerDirection::Store,
                pos: TrackerPostion::FileRange(PostionFileRange {
                    path: self.file_path.to_string_lossy().to_string(),
                    range_begin: *offset,
                    range_end: *offset + *length,
                }),
                flags: 0,
            };
            if let Err(e) = self.tracker.add_position(&request).await {
                if e.code() != BuckyErrorCode::AlreadyExists {
                    error!("add to tracker failed for {}", e);
                    return Err(e);
                }
            };
        }

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
}

impl Clone for LocalChunkWriter {
    fn clone(&self) -> Self {
        Self {
            local_path: self.local_path.clone(),
            ndc: self.ndc.clone(),
            tracker: self.tracker.clone(),
            err: self.err.clone(),
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
            local_path,
            ndc,
            tracker,
            err: Arc::new(Mutex::new(BuckyErrorCode::Ok)),
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

        self.ndc
            .update_chunk_state(&UpdateChunkStateRequest {
                chunk_id: chunk_id.clone(),
                current_state: None,
                state: ChunkState::Ready,
            })
            .await
            .map_err(|e| {
                error!("{} add to tracker failed for {}", self, e);
                e
            })?;

        let request = AddTrackerPositonRequest {
            id: chunk_id.to_string(),
            direction: TrackerDirection::Store,
            pos: TrackerPostion::File(self.local_path.to_string_lossy().to_string()),
            flags: 0,
        };
        if let Err(e) = self.tracker.add_position(&request).await {
            if e.code() != BuckyErrorCode::AlreadyExists {
                error!("add to tracker failed for {}", e);
                return Err(e);
            }
        };

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
