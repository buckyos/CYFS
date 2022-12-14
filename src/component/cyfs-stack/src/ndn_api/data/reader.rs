use cyfs_base::*;
use cyfs_bdt::ChunkReader;
use cyfs_chunk_cache::{ChunkManager, ChunkRead, ChunkType};
use cyfs_util::cache::{
    GetChunkRequest, GetTrackerPositionRequest, NamedDataCache, RemoveTrackerPositionRequest,
    TrackerCache, TrackerDirection, TrackerPostion,
};
use cyfs_util::{AsyncReadWithSeek, ChunkReaderWithHash, ReaderWithLimit};

use async_std::fs::OpenOptions;
use futures::AsyncSeekExt;
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

    async fn read_chunk(
        chunk: &ChunkId,
        path: &Path,
        offset: u64,
    ) -> BuckyResult<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>> {
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

        // async_std::Take not support seek, so use ReaderWithLimit instead
        // let limit_reader = Box::new(file.take(chunk.len() as u64));
        let limit_reader =
            Box::new(ReaderWithLimit::new(chunk.len() as u64, Box::new(file)).await?);

        let hash_reader = ChunkReaderWithHash::new(
            path.to_string_lossy().to_string(),
            chunk.to_owned(),
            limit_reader,
        );

        Ok(Box::new(hash_reader))
    }

    async fn read_impl(
        &self,
        chunk: &ChunkId,
    ) -> BuckyResult<(
        Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        TrackerPostion,
    )> {
        let request = GetTrackerPositionRequest {
            id: chunk.to_string(),
            direction: Some(TrackerDirection::Store),
        };
        let ret = self.tracker.get_position(&request).await?;
        if ret.len() == 0 {
            let msg = format!("chunk not exists: {}", chunk);
            warn!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        } else {
            for c in ret {
                let mut read_indeed = true;
                let read_ret = match &c.pos {
                    //FIXME
                    TrackerPostion::File(path) => {
                        info!("will read chunk from file: chunk={}, file={}", chunk, path);
                        Self::read_chunk(chunk, Path::new(path), 0).await
                    }
                    TrackerPostion::FileRange(fr) => {
                        info!(
                            "will read chunk from file range: chunk={}, file={}, range={}:{}",
                            chunk, fr.path, fr.range_begin, fr.range_end
                        );
                        Self::read_chunk(chunk, Path::new(fr.path.as_str()), fr.range_begin).await
                    }
                    TrackerPostion::ChunkManager => {
                        info!("will read chunk from chunk manager: chunk={}", chunk);
                        let chunk_body = self
                            .chunk_manager
                            .get_chunk(chunk, ChunkType::MemChunk)
                            .await?;
                        let reader = ChunkRead::new(chunk_body);
                        Ok(Box::new(reader) as Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>)
                    }
                    value @ _ => {
                        read_indeed = false;

                        let msg = format!(
                            "unsupport tracker postion for chunk={}, position={:?}",
                            chunk, value,
                        );
                        error!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
                    }
                };

                match read_ret {
                    Ok(content) => {
                        return Ok((content, c.pos));
                    }
                    Err(e) => {
                        if read_indeed {
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
                        }

                        continue;
                    }
                }
            }

            error!("read {} from all tracker position failed", chunk);
            Err(BuckyError::new(
                BuckyErrorCode::NotFound,
                format!("chunk not exists: {}", chunk),
            ))
        }
    }

    /*
    async fn read_to_buf(chunk: &ChunkId, path: &Path, offset: u64) -> BuckyResult<Vec<u8>> {
        let mut reader = Self::read_chunk(chunk, path, offset).await?;

        let mut content = vec![0u8; chunk.len()];
        let read = reader.read(content.as_mut_slice()).await?;

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

        Ok(content)
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
    */
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

    async fn get(
        &self,
        chunk: &ChunkId,
    ) -> BuckyResult<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>> {
        let (reader, _) = self.read_impl(chunk).await?;
        Ok(reader)
    }
}
