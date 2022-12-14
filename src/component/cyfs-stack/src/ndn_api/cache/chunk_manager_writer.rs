use super::state::*;
use cyfs_base::*;
use crate::ndn_api::ChunkWriter;
use cyfs_chunk_cache::{Chunk, ChunkManagerRef, MemRefChunk};
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
    async fn write(&self, chunk_id: &ChunkId, content: &[u8]) -> BuckyResult<()> {
        let ref_chunk = MemRefChunk::from(unsafe {
            std::mem::transmute::<_, &'static [u8]>(content)
        });
        let content = Box::new(ref_chunk) as Box<dyn Chunk>;
        self.chunk_manager
            .put_chunk(chunk_id, content.as_ref())
            .await?;

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
