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
    // download::*
};



struct CacheState {
    // downloader: ChunkDownloader, 
    stream_cache: ChunkStreamCache, 
}

struct CacheImpl {
    stack: WeakStack, 
    chunk: ChunkId, 
    state: RwLock<CacheState>
}

#[derive(Clone)]
pub struct ChunkCache(Arc<CacheImpl>);

impl ChunkCache {
    fn new(stack: WeakStack, chunk: ChunkId) -> Self {
        Self(Arc::new((CacheImpl {
            stack, 
            state: RwLock::new(CacheState {
                // downloader: ChunkDownloader::new(), 
                stream_cache: ChunkStreamCache::new(&chunk), 
            }),
            chunk, 
        })))
    }

    pub fn add_context(&self, context: SingleDownloadContext) {
        unimplemented!()
    }

    pub fn remove_context(&self, context: &SingleDownloadContext) {
        unimplemented!()
    }

    pub async fn read(&self, piece_desc: &PieceDesc, buf: &mut [u8], timeout: Option<Duration>) -> BuckyResult<usize> {
        unimplemented!()
    }
}

