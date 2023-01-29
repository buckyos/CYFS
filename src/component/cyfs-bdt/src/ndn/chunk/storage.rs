use async_trait::async_trait;
use cyfs_base::*;
use cyfs_util::AsyncReadWithSeek;


#[async_trait]
pub trait ChunkReader: 'static + Send + Sync {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader>;
    async fn exists(&self, chunk: &ChunkId) -> bool;
    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>>;
}


