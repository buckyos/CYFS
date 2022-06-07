use cyfs_chunk_lib::*;
use cyfs_base::{BuckyResult, ChunkId};

#[derive(Copy, Clone)]
pub enum ChunkType {
    MemChunk,
    MMapChunk,
}

#[async_trait::async_trait]
pub(crate) trait ChunkCache: Send + Sync
{
    async fn get_chunk(&self, chunk_id: &ChunkId, chunk_type: ChunkType) -> BuckyResult<Box<dyn Chunk>>;
    async fn new_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<Box<dyn ChunkMut>>;
    async fn delete_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<()>;
    async fn put_chunk(&self, chunk_id: &ChunkId, chunk: &dyn Chunk) -> BuckyResult<()>;
    async fn is_exist(&self, chunk_id: &ChunkId) -> bool;
    async fn get_chunk_meta(&self, chunk_id: &ChunkId, chunk_type: ChunkType) -> BuckyResult<ChunkMeta>;
}
