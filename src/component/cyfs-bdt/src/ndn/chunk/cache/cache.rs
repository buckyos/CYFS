use std::{
    sync::{Arc}, 
    ops::Range, 
};
use cyfs_base::*;
use crate::{
    stack::{WeakStack}, 
};
use super::super::super::{
    types::*, 
    channel::protocol::v0::PieceData
};
use super::{
    encode::*, 
    stream::*, 
    download::*
};



struct CacheImpl {
    chunk: ChunkId, 
    downloader: ChunkDownloader, 
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
        let stream_cache = ChunkStreamCache::new(&chunk);
        Self(Arc::new(CacheImpl {
            downloader: ChunkDownloader::new(stack.clone(), chunk.clone(), stream_cache.clone()), 
            chunk, 
            stream_cache, 
        }))
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn downloader(&self) -> &ChunkDownloader {
        &self.0.downloader
    }

    pub fn stream(&self) -> &ChunkStreamCache {
        &self.0.stream_cache
    }

    pub fn create_encoder(&self, desc: &ChunkEncodeDesc) -> Box<dyn ChunkEncoder> {
        StreamEncoder::new(self.stream().clone(), desc).clone_as_encoder()
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

