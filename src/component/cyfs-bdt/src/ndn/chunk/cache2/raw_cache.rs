use std::{
    sync::{Arc, RwLock}, 
    collections::BTreeMap
};
use cyfs_base::*;
use cyfs_util::{
    AsyncWriteWithSeek, 
    AsyncReadWithSeek, 
    SyncWriteWithSeek, 
    SyncReadWithSeek
};
use crate::{
    types::*
};



#[async_trait::async_trait]
pub trait RawCache: Send + Sync {
    fn clone_as_raw_cache(&self) -> Box<dyn RawCache>;
    async fn async_reader(&self) -> BuckyResult<Box<dyn Unpin + Send + Sync + AsyncReadWithSeek>>;
    fn sync_reader(&self) -> BuckyResult<Box<dyn SyncReadWithSeek>>;
    async fn async_writer(&self) -> BuckyResult<Box<dyn  Unpin + Send + Sync + AsyncWriteWithSeek>>;
    fn sync_writer(&self) -> BuckyResult<Box<dyn SyncWriteWithSeek>>;
}

struct MemCacheImpl {
    id: IncreaseId, 
    mem: RwLock<Vec<u8>>
}

#[derive(Clone)]
pub struct RawMemCache(Arc<MemCacheImpl>);

impl RawMemCache {
    fn new(id: IncreaseId, capacity: usize) -> Self {
        Self(Arc::new(MemCacheImpl {
            id, 
            mem: RwLock::new(vec![0u8; capacity])
        }))
    } 

    pub fn id(&self) -> IncreaseId {
        self.0.id
    }
}

#[async_trait::async_trait]
impl RawCache for RawMemCache {
    fn clone_as_raw_cache(&self) -> Box<dyn RawCache> {
        Box::new(self.clone())
    } 

    async fn async_reader(&self) -> BuckyResult<Box<dyn Unpin + Send + Sync + AsyncReadWithSeek>> {
        unimplemented!()
    }

    fn sync_reader(&self) -> BuckyResult<Box<dyn SyncReadWithSeek>> {
        unimplemented!()
    }

    async fn async_writer(&self) -> BuckyResult<Box<dyn Unpin + Send + Sync + AsyncWriteWithSeek>> {
        unimplemented!()
    }

    fn sync_writer(&self) -> BuckyResult<Box<dyn SyncWriteWithSeek>> {
        unimplemented!()
    }
}



struct ManagerImpl {
    mem_caches: BTreeMap<IncreaseId, RawMemCache>, 
}

#[derive(Clone)]
pub struct RawCacheManager(Arc<RwLock<ManagerImpl>>);

impl RawCacheManager {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(ManagerImpl {
            mem_caches: Default::default()
        })))
    }

    pub async fn alloc(&self, capacity: usize) -> Box<dyn RawCache> {
        unimplemented!()
    }

    pub fn alloc_mem(&self, capacity: usize) -> RawMemCache {
        unimplemented!()
    }
}