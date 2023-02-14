use super::state::*;
use super::chunk_writer::ChunkWriter;
use cyfs_base::*;
use cyfs_chunk_cache::{Chunk, ChunkManagerRef};
use cyfs_debug::Mutex;
use cyfs_util::cache::{NamedDataCache, TrackerCache};

use std::sync::Arc;

pub struct ChunkManagerWriter {
    err: Arc<Mutex<Option<BuckyError>>>,
    chunk_manager: ChunkManagerRef,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
}

/*
impl Clone for ChunkManagerWriter {
    fn clone(&self) -> Self {
        Self {
            err: self.err.clone(),
            chunk_manager: self.chunk_manager.clone(),
            ndc: self.ndc.clone(),
            tracker: self.tracker.clone(),
        }
    }
}
*/

impl ChunkManagerWriter {
    pub fn new(
        chunk_manager: ChunkManagerRef,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
    ) -> Self {
        Self {
            err: Arc::new(Mutex::new(None)),
            chunk_manager,
            ndc,
            tracker,
        }
    }
}

#[async_trait::async_trait]
impl ChunkWriter for ChunkManagerWriter {
    async fn write(&self, chunk_id: &ChunkId, chunk: Box<dyn Chunk>) -> BuckyResult<()> {
        info!("will write chunk: chunk={}, len={}", chunk_id, chunk_id.len());

        self.chunk_manager.put_chunk(chunk_id, chunk).await?;

        ChunkManagerStateUpdater::update_chunk_state(&self.ndc, chunk_id).await?;
        ChunkManagerStateUpdater::update_chunk_tracker(&self.tracker, chunk_id).await?;

        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        Ok(())
    }

    async fn err(&self, e: &BuckyError) -> BuckyResult<()> {
        *self.err.lock().unwrap() = Some(e.to_owned());
        Ok(())
    }
}
