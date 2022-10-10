use std::{
    sync::{Arc, RwLock}, usize, 
    io::SeekFrom, 
};
use async_std::{
    pin::Pin, 
    task::{Context, Poll}
};
use cyfs_base::*;
use cyfs_util::{
    AsyncWriteWithSeek, 
    AsyncReadWithSeek, 
    SyncWriteWithSeek, 
    SyncReadWithSeek
};
use super::{
    common::*, 
    manager::*
};

struct CacheImpl {
    manager: RawCacheManager, 
    cache: RwLock<Vec<u8>>
}

impl CacheImpl {
    fn capacity(&self) -> usize {
        self.cache.read().unwrap().len()
    } 
}

impl Drop for CacheImpl {
    fn drop(&mut self) {
        self.manager.release_mem(self.capacity())
    }
}

#[derive(Clone)]
pub struct MemCache(Arc<CacheImpl>);

impl MemCache {
    pub fn new(manager: RawCacheManager, capacity: usize) -> Self {
        Self(Arc::new(CacheImpl {
            manager, 
            cache: RwLock::new(vec![0u8; capacity])
        }))
    }

    fn seek(&self, cur: usize, pos: SeekFrom) -> usize {
        let capacity = self.capacity();
        match pos {
            SeekFrom::Start(offset) => capacity.min(offset as usize), 
            SeekFrom::Current(offset) => {
                let offset = (cur as i64) + offset;
                let offset = offset.max(0);
                capacity.min(offset as usize)
            },
            SeekFrom::End(offset) => {
                let offset = (capacity as i64) + offset;
                let offset = offset.max(0);
                capacity.min(offset as usize)
            }
        }
    }

    fn read(&self, offset: usize, buffer: &mut [u8]) -> usize {
        let capacity = self.capacity();
        let start = offset.min(capacity);
        let end = (offset + buffer.len()).min(capacity);
        let len = end - start;
        if len > 0 {
            buffer[0..len].copy_from_slice(&self.0.cache.read().unwrap()[start..end]);
            len
        } else {
            0
        }
    }

    fn write(&self, offset: usize, buffer: &[u8]) -> usize {
        let capacity = self.capacity();
        let start = offset.min(capacity);
        let end = (offset + buffer.len()).min(capacity);
        let len = end - start;
        if len > 0 {
            self.0.cache.write().unwrap()[start..end].copy_from_slice(&buffer[0..len]);
            len
        } else {
            0
        }
    }
}

struct SeekWrapper {
    cache: MemCache, 
    offset: usize
}

impl SeekWrapper {
    fn new(cache: &MemCache) -> Self {
        Self {
            cache: cache.clone(), 
            offset: 0
        }
    }
}

impl async_std::io::Seek for SeekWrapper {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        let pined = self.get_mut();
        pined.offset = pined.cache.seek(pined.offset, pos);
        Poll::Ready(Ok(pined.offset as u64)) 
    }
}

impl async_std::io::Read for SeekWrapper {
    fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
        let pined = self.get_mut();
        let read = pined.cache.read(pined.offset, buf);
        pined.offset += read;
        Poll::Ready(Ok(read))
    }
}

impl AsyncReadWithSeek for SeekWrapper {}

impl std::io::Seek for SeekWrapper {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.offset = self.cache.seek(self.offset, pos);
        Ok(self.offset as u64)
    }
}

impl std::io::Read for SeekWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.cache.read(self.offset, buf);
        self.offset += read;
        Ok(read)
    }
}

impl SyncReadWithSeek for SeekWrapper {}

impl async_std::io::Write for SeekWrapper {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let pined = self.get_mut();
        let written = pined.cache.write(pined.offset, buf);
        pined.offset += written;
        Poll::Ready(Ok(written))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncWriteWithSeek for SeekWrapper {}

impl std::io::Write for SeekWrapper {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.cache.write(self.offset, buf);
        self.offset += written;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SyncWriteWithSeek for SeekWrapper {}

#[async_trait::async_trait]
impl RawCache for MemCache {
    fn capacity(&self) -> usize {
        self.0.capacity()
    }

    fn clone_as_raw_cache(&self) -> Box<dyn RawCache> {
        Box::new(self.clone())
    }

    async fn async_reader(&self) -> BuckyResult<Box<dyn Unpin + Send + Sync + AsyncReadWithSeek>> {
        Ok(Box::new(SeekWrapper::new(self)))
    }

    fn sync_reader(&self) -> BuckyResult<Box<dyn SyncReadWithSeek>> {
        Ok(Box::new(SeekWrapper::new(self)))
    }
    
    async fn async_writer(&self) -> BuckyResult<Box<dyn  Unpin + Send + Sync + AsyncWriteWithSeek>> {
        Ok(Box::new(SeekWrapper::new(self)))
    }   

    fn sync_writer(&self) -> BuckyResult<Box<dyn SyncWriteWithSeek>> {
        Ok(Box::new(SeekWrapper::new(self)))
    }
}


