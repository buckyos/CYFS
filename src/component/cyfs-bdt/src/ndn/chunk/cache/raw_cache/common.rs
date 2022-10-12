use cyfs_base::*;
use cyfs_util::{
    AsyncWriteWithSeek, 
    AsyncReadWithSeek, 
    SyncWriteWithSeek, 
    SyncReadWithSeek
};


#[async_trait::async_trait]
pub trait RawCache: Send + Sync {
    fn capacity(&self) -> usize;
    fn clone_as_raw_cache(&self) -> Box<dyn RawCache>;
    async fn async_reader(&self) -> BuckyResult<Box<dyn Unpin + Send + Sync + AsyncReadWithSeek>>;
    fn sync_reader(&self) -> BuckyResult<Box<dyn SyncReadWithSeek>>;
    async fn async_writer(&self) -> BuckyResult<Box<dyn  Unpin + Send + Sync + AsyncWriteWithSeek>>;
    fn sync_writer(&self) -> BuckyResult<Box<dyn SyncWriteWithSeek>>;
}



#[derive(Clone)]
pub struct RawCacheConfig {
    
}
