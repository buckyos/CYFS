use std::{
    sync::{Arc, RwLock}, 
    collections::BTreeMap, 
    time::Duration
};
use cyfs_base::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}
};
use super::super::super::{
    types::*, 
    download::*, 
};
use super::{
    encode::*, 
    stream::*, 
    download::*
};



struct CacheImpl {
    stack: WeakStack, 
    chunk: ChunkId, 
    downloader: ChunkDownloader, 
    stream_cache: ChunkStreamCache, 
}

#[derive(Clone)]
pub struct ChunkCache(Arc<CacheImpl>);

impl ChunkCache {
    pub fn new(stack: WeakStack, chunk: ChunkId) -> Self {
        let stream_cache = ChunkStreamCache::new(&chunk);
        Self(Arc::new((CacheImpl {
            downloader: ChunkDownloader::new(stack.clone(), chunk.clone(), stream_cache.clone()), 
            chunk, 
            stack, 
            stream_cache, 
        })))
    }

    pub fn downloader(&self) -> &ChunkDownloader {
        &self.0.downloader
    }

    pub fn stream(&self) -> &ChunkStreamCache {
        &self.0.stream_cache
    }
    
    pub fn read<T: futures::Future<Output=BuckyError>>(&self, offset: usize, buffer: &mut [u8], abort: T) -> BuckyResult<usize> {
        unimplemented!()
    }
}

