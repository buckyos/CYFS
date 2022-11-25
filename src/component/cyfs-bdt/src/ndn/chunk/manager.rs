use std::{
    collections::{BTreeMap}, 
    sync::{RwLock},
};
use async_std::{
    io::Cursor
};
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_util::*;
use crate::{
    stack::{WeakStack, Stack},
};
use super::{
    storage::*,  
    cache::*,
    download::*
};

#[derive(Clone)]
pub struct Config {
    pub raw_caches: RawCacheConfig
}

#[derive(Clone)]
pub struct DownloadingChunkCache {
    cache: ChunkCache, 
    downloader: ChunkDownloader
}

impl DownloadingChunkCache {
    pub fn cache(&self) -> &ChunkCache{
        &self.cache
    }

    pub fn downloader(&self) -> &ChunkDownloader {
        &self.downloader
    }
}


pub struct ChunkManager {
    stack: WeakStack, 
    store: Box<dyn ChunkReader>, 
    raw_caches: RawCacheManager, 
    chunk_caches: RwLock<BTreeMap<ChunkId, DownloadingChunkCache>>, 
}

impl std::fmt::Display for ChunkManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkManager:{{local:{}}}", Stack::from(&self.stack).local_device_id())
    }
}


struct EmptyChunkWrapper(Box<dyn ChunkReader>);

impl EmptyChunkWrapper {
    fn new(non_empty: Box<dyn ChunkReader>) -> Self {
        Self(non_empty)
    }
}

#[async_trait]
impl ChunkReader for EmptyChunkWrapper {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(Self(self.0.clone_as_reader()))
    }

    async fn exists(&self, chunk: &ChunkId) -> bool {
        if chunk.len() == 0 {
            true
        } else {
            self.0.exists(chunk).await
        }
    }

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>> {
        if chunk.len() == 0 {
            Ok(Box::new(Cursor::new(vec![0u8; 0])))
        } else {
            self.0.get(chunk).await
        }
    }
}



impl ChunkManager {
    pub(crate) fn new(
        weak_stack: WeakStack, 
        store: Box<dyn ChunkReader>
    ) -> Self {
        let stack = Stack::from(&weak_stack);
        Self { 
            stack: weak_stack, 
            store: Box::new(EmptyChunkWrapper::new(store)), 
            raw_caches: RawCacheManager::new(stack.config().ndn.chunk.raw_caches.clone()), 
            chunk_caches: RwLock::new(Default::default())
        }
    }

    pub fn store(&self) -> &dyn ChunkReader {
        self.store.as_ref()
    }

    pub fn raw_caches(&self) -> &RawCacheManager {
        &self.raw_caches
    }

    pub fn create_cache(&self, chunk: &ChunkId) -> DownloadingChunkCache {
        let mut caches = self.chunk_caches.write().unwrap();
        if let Some(cache) = caches.get(chunk).cloned() {
            cache
        } else {
            let cache = ChunkCache::new(self.stack.clone(), chunk.clone());
            let downloader = ChunkDownloader::new(self.stack.clone(), chunk.clone(), cache.clone());
            let downloading_cache = DownloadingChunkCache {
                cache, 
                downloader
            };
            caches.insert(chunk.clone(), downloading_cache.clone());
            downloading_cache
        }
    }

    pub fn cache_of(&self, chunk: &ChunkId) -> Option<DownloadingChunkCache> {
        self.chunk_caches.read().unwrap().get(chunk).cloned()
    }

}