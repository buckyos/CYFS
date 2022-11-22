use std::{
    sync::{Arc, RwLock}, usize, 
    io::SeekFrom, 
    ops::Range, 
    path::{Path, PathBuf}
};
use async_std::{
    pin::Pin, 
    task::{self, Context, Poll},
    fs
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
use super::{
    common::*, 
};

enum CacheState {
    Creating(StateWaiter),
    Created(async_std::fs::File), 
    Error(BuckyError)
}

struct CacheImpl {
    state: RwLock<CacheState>, 
    path: PathBuf,
    range: Range<u64>, 
    to_remove: bool
}


impl Drop for FileCache {
    fn drop(&mut self) {
        if self.0.to_remove {
            let to_remove = {
                let state = &mut *self.0.state.write().unwrap();
                let to_remove = match state {
                    CacheState::Created(_) => true, 
                    _ => false
                };
                *state = CacheState::Error(BuckyError::new(BuckyErrorCode::NotInit, "closed"));
                to_remove
            };
            
            if to_remove {
                let path = self.0.path.clone();
                task::spawn(async move {
                    let _ = fs::remove_file(path).await;
                });
            }
        }
    }
}


#[derive(Clone)]
pub struct FileCache(Arc<CacheImpl>);


impl FileCache {
    pub fn from_path(path: PathBuf, range: Range<u64>) -> Self {
        Self::new(path, range, false)
    }

    pub(super) fn new(path: PathBuf, range: Range<u64>, to_remove: bool) -> Self {
        let cache = Self(Arc::new(CacheImpl {
            state: RwLock::new(CacheState::Creating(StateWaiter::new())), 
            path,
            range, 
            to_remove
        }));
        
        {
            let cache = cache.clone();
            task::spawn(async move {
                let ret = cache.create().await;

                let new_state = match ret {
                    Ok(file) => CacheState::Created(file), 
                    Err(err) => CacheState::Error(err)
                };
                let waiters = {
                    let state = &mut *cache.0.state.write().unwrap();
                    match state {
                        CacheState::Creating(waiters) => {
                            let waiters = waiters.transfer();
                            *state = new_state;
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

    async fn create(&self) -> BuckyResult<async_std::fs::File> {
        let mut file = async_std::fs::File::open(self.path()).await?;
        use async_std::io::prelude::SeekExt;
        let offset = file.seek(SeekFrom::Start(self.0.range.start)).await?;
        if offset == self.range().start {
            Ok(file)
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidData,"offset to range failed"))
        }
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

    fn path(&self) -> &Path {
        self.0.path.as_path()
    }

    fn range(&self) -> &Range<u64> {
        &self.0.range
    }


    fn is_created(&self) -> BuckyResult<async_std::fs::File> {
        match &*self.0.state.read().unwrap() {
            CacheState::Creating(_) => Err(BuckyError::new(BuckyErrorCode::WouldBlock, "")), 
            CacheState::Created(file) => Ok(file.clone()), 
            CacheState::Error(err) => Err(err.clone())
        }
    }

    async fn wait_created(&self) -> BuckyResult<async_std::fs::File> {
        let (ret, waiter) = {
            match &mut *self.0.state.write().unwrap() {
                CacheState::Creating(waiters) => (None, Some(waiters.new_waiter())),
                CacheState::Created(file) => (Some(Ok(file.clone())), None), 
                CacheState::Error(err) => (Some(Err(err.clone())), None)
            }
        };
        if let Some(ret) = ret {
            ret 
        } else if let Some(waiter) = waiter {
            StateWaiter::wait(waiter, || {
                match &*self.0.state.read().unwrap() {
                    CacheState::Creating(_) => unreachable!(),
                    CacheState::Created(file) => Ok(file.clone()), 
                    CacheState::Error(err) => Err(err.clone())
                }
            }).await
        } else {
            unreachable!()
        }
    }
}


pub struct FileCacheAsyncReader {
    file: async_std::fs::File, 
    cache: FileCache, 
    offset: usize
}


impl async_std::io::Seek for FileCacheAsyncReader {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        let reader = self.get_mut();
        let file_offset = reader.cache.seek(reader.offset, pos) as u64 + reader.cache.range().start;

        let ret = async_std::io::Seek::poll_seek(Pin::new(&mut reader.file), cx, SeekFrom::Start(file_offset));

        match ret {
            Poll::Ready(ret) => {
                match ret {
                    Ok(file_offset) => {
                        let offset = file_offset - reader.cache.range().start;
                        reader.offset = offset as usize;
                        Poll::Ready(Ok(offset))
                    }, 
                    Err(err) => Poll::Ready(Err(err))
                } 
            },
            Poll::Pending => Poll::Pending
        }
    }
}

impl async_std::io::Read for FileCacheAsyncReader {
    fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
        let reader = self.get_mut();
        let new_offset = reader.cache.seek(reader.offset, SeekFrom::Current(buf.len() as i64));
        let cliped = &mut buf[0..new_offset - reader.offset];

        let ret = async_std::io::Read::poll_read(Pin::new(&mut reader.file), cx, cliped);

        match ret {
            Poll::Ready(ret) => {
                match ret {
                    Ok(read) => {
                        reader.offset += read;
                        Poll::Ready(Ok(read))
                    }, 
                    Err(err) => Poll::Ready(Err(err))
                }
            },
            Poll::Pending => Poll::Pending
        }
    }
}

impl AsyncReadWithSeek for FileCacheAsyncReader {}


pub struct FileCacheSyncReader {
    file: std::fs::File, 
    cache: FileCache, 
    offset: usize
}


impl std::io::Seek for FileCacheSyncReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let file_offset = self.cache.seek(self.offset, pos) as u64 + self.cache.range().start;

        let file_offset = std::io::Seek::seek(&mut self.file, SeekFrom::Start(file_offset))?;

        let offset = file_offset - self.cache.range().start;

        self.offset = offset as usize;

        Ok(offset)
    }
}

impl std::io::Read for FileCacheSyncReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let new_offset = self.cache.seek(self.offset, SeekFrom::Current(buf.len() as i64));
        let cliped = &mut buf[0..new_offset - self.offset];

        let read = std::io::Read::read(&mut self.file, cliped)?;

        self.offset += read;
        Ok(read)
    }
}

impl SyncReadWithSeek for FileCacheSyncReader {}

#[async_trait::async_trait]
impl RawCache for FileCache {
    fn capacity(&self) -> usize {
        (self.range().end - self.range().start) as usize
    }

    fn clone_as_raw_cache(&self) -> Box<dyn RawCache> {
        Box::new(self.clone())
    }

    async fn async_reader(&self) -> BuckyResult<Box<dyn Unpin + Send + Sync + AsyncReadWithSeek>> {
        let file = self.wait_created().await?;
        
        Ok(Box::new(FileCacheAsyncReader {
            file, 
            cache: self.clone(),
            offset: 0
        }))
    }

    fn sync_reader(&self) -> BuckyResult<Box<dyn SyncReadWithSeek + Send + Sync>> {
        let _ = self.is_created()?;

        let mut file = std::fs::File::open(self.path())?;

        use std::io::Seek;
        let offset = file.seek(SeekFrom::Start(self.range().start))?;
        if offset == self.range().start {
            Ok(Box::new(FileCacheSyncReader {
                file, 
                cache: self.clone(),
                offset: 0
            }))
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidData,"offset to range failed"))
        }   
    }
    
    async fn async_writer(&self) -> BuckyResult<Box<dyn  Unpin + Send + Sync + AsyncWriteWithSeek>> {
        Err(BuckyError::new(BuckyErrorCode::NotSupport, "file cache does not support sync reader"))
    }   

    fn sync_writer(&self) -> BuckyResult<Box<dyn SyncWriteWithSeek>> {
        Err(BuckyError::new(BuckyErrorCode::NotSupport, "file cache does not support sync reader"))
    }
}


