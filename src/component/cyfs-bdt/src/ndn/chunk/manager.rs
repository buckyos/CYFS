use std::{
    collections::{BTreeMap, LinkedList}, 
    sync::{Mutex},
};
use async_std::{
    io::Cursor
};
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_util::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack},
};
use super::super::{
    download::*
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

struct Downloaders(LinkedList<WeakChunkDownloader>);

impl Downloaders {
    fn new() -> Self {
        Self(Default::default())
    }

    fn create_downloader(
        &mut self, 
        stack: &WeakStack, 
        cache: ChunkCache, 
        task: Box<dyn LeafDownloadTask>
    ) -> ChunkDownloader {
        let downloader = ChunkDownloader::new(stack.clone(), cache, task);
        self.0.push_back(downloader.to_weak());
        downloader
    }

    fn get_all(&mut self) -> LinkedList<ChunkDownloader> {
        let mut all = LinkedList::new();
        let mut remain = LinkedList::new();
        for weak in &self.0 {
            if let Some(downloader) = weak.to_strong() {
                remain.push_back(weak.clone());
                all.push_back(downloader);
            } 
        }
        std::mem::swap(&mut self.0, &mut remain);
        all
    }
}


pub struct ChunkManager {
    stack: WeakStack, 
    store: Box<dyn ChunkReader>, 
    raw_caches: RawCacheManager, 
    caches: Mutex<BTreeMap<ChunkId, WeakChunkCache>>, 
    downloaders: Mutex<Downloaders>
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
            caches: Mutex::new(Default::default()), 
            downloaders: Mutex::new(Downloaders::new())
        }
    }

    pub(crate) fn on_statistic(&self) -> String {
        let caches = self.caches.lock().unwrap();
        format!("ChunkCacheCount:{}",  caches.len())
    }

    pub fn store(&self) -> &dyn ChunkReader {
        self.store.as_ref()
    }

    pub fn raw_caches(&self) -> &RawCacheManager {
        &self.raw_caches
    }

    pub fn create_cache(&self, chunk: &ChunkId) -> ChunkCache {
        let mut caches = self.caches.lock().unwrap();
        if let Some(weak) = caches.get(chunk) {
            if let Some(cache) = weak.to_strong().clone() {
                return cache;
            }
            caches.remove(chunk);
        } 
        let cache = ChunkCache::new(self.stack.clone(), chunk.clone());
        info!("{} create new cache {}", self, cache);
        caches.insert(chunk.clone(), cache.to_weak());
        cache
    }

    pub fn create_downloader(&self, chunk: &ChunkId, task: Box<dyn LeafDownloadTask>) -> ChunkDownloader {
        let cache = self.create_cache(chunk);
        let mut downloaders = self.downloaders.lock().unwrap();
        let downloader = downloaders.create_downloader(&self.stack, cache, task);
        info!("{} create new downloader {}", self, downloader);
        downloader
    }

    pub(in super::super) fn on_schedule(&self, _now: Timestamp) {
        let downloaders = {
            let mut downloaders = self.downloaders.lock().unwrap();
            downloaders.get_all()
        };
        for downloader in downloaders {
            downloader.on_drain(0);
        }
    } 
}