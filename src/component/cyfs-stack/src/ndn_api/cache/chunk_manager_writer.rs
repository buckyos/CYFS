use super::state::*;
use cyfs_base::*;
use cyfs_bdt::ChunkWriter;
use cyfs_chunk_cache::{Chunk, ChunkManagerRef, MemRefChunk};
use cyfs_debug::Mutex;
use cyfs_util::cache::{NamedDataCache, TrackerCache};

use std::fmt::{Display, Formatter};
use std::sync::Arc;


pub struct ChunkManagerWriter {
    err: Arc<Mutex<BuckyErrorCode>>,
    chunk_manager: ChunkManagerRef,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
}

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

impl ChunkManagerWriter {
    pub fn new(
        chunk_manager: ChunkManagerRef,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
    ) -> Self {
        Self {
            err: Arc::new(Mutex::new(BuckyErrorCode::Ok)),
            chunk_manager,
            ndc,
            tracker,
        }
    }
}

impl Display for ChunkManagerWriter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkManager writer")
    }
}

#[async_trait::async_trait]
impl ChunkWriter for ChunkManagerWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone())
    }

    async fn write(&self, chunk_id: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        let ref_chunk = MemRefChunk::from(unsafe {
            std::mem::transmute::<_, &'static [u8]>(content.as_slice())
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

    async fn err(&self, e: BuckyErrorCode) -> BuckyResult<()> {
        *self.err.lock().unwrap() = e;
        Ok(())
    }
}
