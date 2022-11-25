use std::{
    sync::{Arc, Mutex}, 
    ops::Range, 
};
use async_std::{
    task
};
use cyfs_base::*;
use crate::{
    types::*,
    stack::{Stack, WeakStack}
};
use super::super::super::{
    types::*, 
    channel::protocol::v0::PieceData,
};
use super::super::{
    storage::*
};
use super::{
    encode::*, 
    stream::*, 
    raw_cache::*
};

enum CacheState {
    Loading(StateWaiter),
    Loaded(bool)
}

struct CacheImpl {
    chunk: ChunkId,  
    state: Mutex<CacheState>, 
    stream_cache: ChunkStreamCache, 
}

#[derive(Clone)]
pub struct ChunkCache(Arc<CacheImpl>);


impl std::fmt::Display for ChunkCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkCache{{chunk:{}}}", self.chunk())
    }
}

impl ChunkCache {
    pub fn new(stack: WeakStack, chunk: ChunkId) -> Self {
        let cache = Self(Arc::new(CacheImpl {
            stream_cache: ChunkStreamCache::new(&chunk), 
            chunk, 
            state: Mutex::new(CacheState::Loading(StateWaiter::new()))
        }));

        {
            let stack = Stack::from(&stack);
            let cache = cache.clone();

            task::spawn(async move {
                let raw_cache = stack.ndn().chunk_manager().raw_caches().alloc(cache.chunk().len()).await;
                let finished = cache.load(raw_cache.as_ref(), stack.ndn().chunk_manager().store()).await.is_ok();
                let _ = cache.stream().load(finished, raw_cache);
                let waiters = {
                    let state = &mut *cache.0.state.lock().unwrap();
                    match state {
                        CacheState::Loading(waiters) => {
                            let waiters = waiters.transfer(); 
                            *state = CacheState::Loaded(finished);
                            waiters
                        },
                        _ => unreachable!()
                    }
                };
                waiters.wake();
            });
        }
        
        cache
    }


    async fn load(&self, cache: &dyn RawCache, storage: &dyn ChunkReader) -> BuckyResult<()> {
        let reader = storage.get(self.chunk()).await?;

        let writer = cache.async_writer().await?;

        let written = async_std::io::copy(reader, writer).await? as usize;
        
        if written != self.chunk().len() {
            Err(BuckyError::new(BuckyErrorCode::InvalidInput, ""))
        } else {
            Ok(())
        }
    } 

    pub async fn wait_loaded(&self) -> bool {
        let (waiter, finished) = {
            let state = &mut *self.0.state.lock().unwrap();
            match state {
                CacheState::Loading(waiters) => (Some(waiters.new_waiter()), None), 
                CacheState::Loaded(finished) => (None, Some(*finished))
            }
        };

        if let Some(waiter) = waiter {
            StateWaiter::wait(waiter, || {
                let state = &*self.0.state.lock().unwrap();
                if let CacheState::Loaded(finished) = state {
                    *finished
                } else {
                    unreachable!()
                }
            }).await
        } else {
            finished.unwrap()
        }
        
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn stream(&self) -> &ChunkStreamCache {
        &self.0.stream_cache
    }

    pub fn create_encoder(&self, desc: &ChunkEncodeDesc) -> Box<dyn ChunkEncoder> {
        self.stream().create_encoder(desc).clone_as_encoder()
    }

    pub fn exists(&self, range: Range<usize>) -> Option<Range<usize>> {
        if range.start >= self.chunk().len() {
            return Some(self.chunk().len()..self.chunk().len());
        }
        if range.end == 0 {
            return Some(0..0);
        }
        let range = usize::min(range.start, self.chunk().len())..usize::min(range.end, self.chunk().len());
        let index_start = (range.start / PieceData::max_payload()) as u32;
        let index_end = ((range.end - 1) / PieceData::max_payload()) as u32;
        for index in index_start..index_end + 1 {
            if !self.stream().exists(index).unwrap() {
                return None;
            }
        }
        return Some(range);
    }

    pub async fn wait_exists<T: futures::Future<Output=BuckyError>, A: Fn() -> T>(
        &self, 
        range: Range<usize>, 
        abort: A
    ) -> BuckyResult<Range<usize>> {
        trace!("{} wait_exists {:?}", self, range);
        if range.start >= self.chunk().len() {
            let r = self.chunk().len()..self.chunk().len();
            trace!("{} wait_exists {:?} return {:?}", self, range, r);
            return Ok(r);
        }
        if range.end == 0 {
            let r = 0..0;
            trace!("{} wait_exists {:?} return {:?}", self, range, r);
            return Ok(r);
        }
        let range = usize::min(range.start, self.chunk().len())..usize::min(range.end, self.chunk().len());
        let index_start = (range.start / PieceData::max_payload()) as u32;
        let index_end = ((range.end - 1) / PieceData::max_payload()) as u32;
        for index in index_start..index_end + 1 {
            self.stream().wait_exists(index, abort()).await?;
        }
        trace!("{} wait_exists {:?} return {:?}", self, range, range);
        Ok(range)
    }
    
    pub async fn read<T: futures::Future<Output=BuckyError>, A: Fn() -> T>(
        &self, 
        offset: usize, 
        buffer: &mut [u8], 
        abort: A
    ) -> BuckyResult<usize> {
        let (desc, mut offset) = PieceDesc::from_stream_offset(PieceData::max_payload(), offset as u32);
        let (mut index, range) = desc.unwrap_as_stream();
        let mut read = 0;
        loop {
            let this_read = self.stream().async_read(
                &PieceDesc::Range(index, range), 
                offset as usize, 
                &mut buffer[read..], 
                abort()).await?;
            read += this_read;
            if this_read == 0 
                || read >= buffer.len() {
                break;
            }
            index += 1;
            offset = 0;
        }
        Ok(read)
    }
}

