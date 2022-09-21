use std::{
    sync::{Arc, RwLock}, 
    collections::BTreeMap
};
use cyfs_base::*;

struct CacheImpl {
    
}

#[derive(Clone)]
pub struct ChunkCache(Arc<CacheImpl>);

impl ChunkCache {
    pub fn encoder_of(&self) {

    }

    pub fn decoder_of(&self) {

    }
}

struct ManagerImpl {
    entries: RwLock<BTreeMap<ChunkId, ChunkCache>>
}

#[derive(Clone)]
pub struct ChunkCacheManager(Arc<ManagerImpl>);

impl ChunkCacheManager {

}