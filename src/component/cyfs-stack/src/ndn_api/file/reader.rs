use cyfs_base::*;
use cyfs_bdt::ChunkReader;
use cyfs_chunk_cache::{ChunkManager, ChunkType};
use cyfs_util::cache::{
    GetChunkRequest, GetTrackerPositionRequest, NamedDataCache, RemoveTrackerPositionRequest,
    TrackerCache, TrackerDirection, TrackerPostion,
};

use async_std::fs::OpenOptions;
use futures::{AsyncReadExt, AsyncSeekExt};
use std::io::SeekFrom;
use std::path::Path;
use std::sync::Arc;

pub struct ChunkStoreReader {
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    chunk_manager: Arc<ChunkManager>,
}

impl Clone for ChunkStoreReader {
    fn clone(&self) -> Self {
        Self {
            ndc: self.ndc.clone(),
            tracker: self.tracker.clone(),
            chunk_manager: self.chunk_manager.clone(),
        }
    }
}

impl ChunkStoreReader {
    pub fn new(
        chunk_manager: Arc<ChunkManager>,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
    ) -> Self {
        Self {
            ndc,
            tracker,
            chunk_manager,
        }
    }

    async fn read_chunk_from_file(
        chunk: &ChunkId,
        path: &Path,
        offset: u64,
    ) -> BuckyResult<Vec<u8>> {
        debug!("begin read {} from file {:?}", chunk, path);
        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .await
            .map_err(|e| {
                let msg = format!("open file {:?} failed for {}", path, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let actual_offset = file.seek(SeekFrom::Start(offset)).await.map_err(|e| {
            let msg = format!("seek file {:?} to offset {} failed for {}", path, offset, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        if actual_offset != offset {
            let msg = format!(
                "seek file {:?} to offset {} actual offset {}",
                path, offset, actual_offset
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }

        let mut content = vec![0u8; chunk.len()];

        let read = file.read(content.as_mut_slice()).await?;

        if read != content.len() {
            let msg = format!(
                "read {} bytes from file {:?} but chunk len is {}",
                read,
                path,
                content.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }

        let actual_id = ChunkId::calculate(content.as_slice()).await?;

        if actual_id.eq(chunk) {
            debug!("read {} from file {:?}", chunk, path);
            Ok(content)
        } else {
            let msg = format!("content in file {:?} not match chunk id", path);
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        }
    }

    async fn is_chunk_stored_in_file(&self, chunk: &ChunkId, path: &Path) -> BuckyResult<bool> {
        let request = GetTrackerPositionRequest {
            id: chunk.to_string(),
            direction: Some(TrackerDirection::Store),
        };
        let ret = self.tracker.get_position(&request).await?;
        if ret.len() == 0 {
            Ok(false)
        } else {
            for c in ret {
                match &c.pos {
                    TrackerPostion::File(exists) => {
                        if path.eq(Path::new(exists)) {
                            return Ok(true);
                        }
                    }
                    TrackerPostion::FileRange(fr) => {
                        if path.eq(Path::new(&fr.path)) {
                            return Ok(true);
                        }
                    }
                    _ => {}
                }
            }
            Ok(false)
        }
    }
}

#[async_trait::async_trait]
impl ChunkReader for ChunkStoreReader {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(self.clone())
    }

    async fn exists(&self, chunk: &ChunkId) -> bool {
        let request = GetChunkRequest {
            chunk_id: chunk.clone(),
            flags: 0,
        };
        match self.ndc.get_chunk(&request).await {
            Ok(c) => {
                if let Some(c) = c {
                    c.state == ChunkState::Ready
                } else {
                    false
                }
            }
            Err(e) => {
                error!("got chunk state {} from database failed for {}", chunk, e);
                false
            }
        }
    }

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Arc<Vec<u8>>> {
        let request = GetTrackerPositionRequest {
            id: chunk.to_string(),
            direction: Some(TrackerDirection::Store),
        };
        let ret = self.tracker.get_position(&request).await?;
        if ret.len() == 0 {
            Err(BuckyError::new(
                BuckyErrorCode::NotFound,
                "chunk not exists",
            ))
        } else {
            for c in ret {
                let read_ret = match &c.pos {
                    //FIXME
                    TrackerPostion::File(path) => {
                        Self::read_chunk_from_file(chunk, Path::new(path), 0).await
                    }
                    TrackerPostion::FileRange(fr) => {
                        Self::read_chunk_from_file(
                            chunk,
                            Path::new(fr.path.as_str()),
                            fr.range_begin,
                        )
                        .await
                    }
                    TrackerPostion::ChunkManager => {
                        let chunk_body = self
                            .chunk_manager
                            .get_chunk(chunk, ChunkType::MemChunk)
                            .await?;
                        Ok(chunk_body.into_vec())
                    }
                    _ => Err(BuckyError::new(
                        BuckyErrorCode::InvalidFormat,
                        "unsupport reader",
                    )),
                };

                match read_ret {
                    Ok(content) => {
                        return Ok(Arc::new(content));
                    }
                    Err(e) => {
                        // 如果tracker中的pos无法正确读取，从tracker中删除这条记录
                        let _ = self
                            .tracker
                            .remove_position(&RemoveTrackerPositionRequest {
                                id: chunk.to_string(),
                                direction: Some(TrackerDirection::Store),
                                pos: Some(c.pos.clone()),
                            })
                            .await;
                        error!(
                            "read {} from tracker position {:?} failed for {}",
                            chunk, c.pos, e
                        );
                        continue;
                    }
                }
            }

            error!("read {} from all tracker position failed", chunk);
            Err(BuckyError::new(
                BuckyErrorCode::NotFound,
                "chunk not exists",
            ))
        }
    }
}
