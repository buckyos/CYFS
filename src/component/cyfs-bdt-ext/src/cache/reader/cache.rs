use cyfs_base::*;
use cyfs_bdt::{chunk::ChunkCache, DownloadTaskSplitRead};
use cyfs_chunk_cache::ChunkManagerRef;
use cyfs_chunk_lib::*;

use async_std::io::{Read, Result};
use std::io::{Seek, SeekFrom};
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait ChunkSplitReader: DownloadTaskSplitRead + Seek {}

impl<T: DownloadTaskSplitRead + Seek> ChunkSplitReader for T {}

pub struct ChunkListCacheReader {
    task_id: String,
    reader: Box<dyn ChunkSplitReader + Unpin + Send + Sync + 'static>,

    chunk_manager: ChunkManagerRef,
    last_cache_chunk: Option<ChunkId>,
    full_len: u64,
    read_len: u64,
}

impl ChunkListCacheReader {
    pub fn new(
        chunk_manager: ChunkManagerRef,
        task_id: String,
        full_len: u64,
        reader: Box<dyn ChunkSplitReader + Unpin + Send + Sync + 'static>,
    ) -> Self {
        Self {
            task_id,
            reader,
            chunk_manager,
            last_cache_chunk: None,
            full_len,
            read_len: 0,
        }
    }

    fn try_cache_chunk(&mut self, cache: &ChunkCache) {
        let mut need_cache = true;
        let chunk_id = cache.chunk();
        match &self.last_cache_chunk {
            Some(prev) => {
                if prev == chunk_id {
                    need_cache = false;
                }
            }
            None => {}
        }

        if !need_cache {
            return;
        }

        let range = std::ops::Range {
            start: 0,
            end: chunk_id.len(),
        };
        if cache.exists(range.clone()) != Some(range) {
            return;
        }

        self.last_cache_chunk = Some(chunk_id.to_owned());

        let cache = cache.clone();
        let chunk_manager = self.chunk_manager.clone();
        async_std::task::spawn(async move {
            let ret = chunk_manager.exist(cache.chunk()).await;
            if ret {
                debug!(
                    "cache chunk to chunk manager but already exists! chunk={}",
                    cache.chunk()
                );
                return;
            }

            let cache_wrapper = ChunkCacheWrapper::new(cache.clone());
            match chunk_manager
                .put_chunk(cache.chunk(), Box::new(cache_wrapper))
                .await
            {
                Ok(()) => {
                    info!(
                        "cache chunk to chunk manager success! chunk={}",
                        cache.chunk()
                    );
                }
                Err(e) => {
                    info!(
                        "cache chunk to chunk manager failed! chunk={}, {}",
                        cache.chunk(),
                        e
                    );
                }
            }
        });
    }
}

impl Read for ChunkListCacheReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        match Pin::new(self.reader.as_mut()).poll_split_read(cx, buf) {
            Poll::Ready(ret) => match ret {
                Ok(Some((cache, range))) => {
                    self.read_len += range.len() as u64;
                    self.try_cache_chunk(&cache);
                    Poll::Ready(Ok(range.len()))
                }
                Ok(None) => {
                    if self.full_len != self.read_len {
                        error!(
                            "unmatch data size got from bdt: data={}, len={}, got={}",
                            self.task_id, self.full_len, self.read_len
                        );
                    }
                    Poll::Ready(Ok(0))
                }
                Err(e) => {
                    warn!(
                        "read data from bdt got error! data={}, len={}, {}",
                        self.task_id, self.full_len, e
                    );
                    Poll::Ready(Err(e))
                }
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Seek for ChunkListCacheReader {
    fn seek(self: &mut Self, pos: SeekFrom) -> std::io::Result<u64> {
        Pin::new(self.reader.as_mut()).seek(pos)
    }
}

struct ChunkCacheWrapper {
    offset: usize,
    cache: ChunkCache,
}

impl ChunkCacheWrapper {
    pub fn new(cache: ChunkCache) -> Self {
        Self { offset: 0, cache }
    }

    pub fn chunk(&self) -> &ChunkId {
        self.cache.chunk()
    }
}

impl std::ops::Deref for ChunkCacheWrapper {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unreachable!();
    }
}

#[async_trait::async_trait]
impl Chunk for ChunkCacheWrapper {
    fn calculate_id(&self) -> ChunkId {
        self.cache.chunk().to_owned()
    }

    fn get_chunk_meta(&self) -> ChunkMeta {
        unreachable!();
    }

    fn get_len(&self) -> usize {
        self.cache.chunk().len()
    }

    fn into_vec(self: Box<Self>) -> Vec<u8> {
        unreachable!();
    }

    async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        let len = self
            .cache
            .read(self.offset, buf, || std::future::pending())
            .await?;
        self.offset += len;
        Ok(len)
    }

    async fn seek(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        let len = self.cache.chunk().len();
        match pos {
            SeekFrom::Start(offset) => {
                if offset as usize > len {
                    self.offset = len;
                } else {
                    self.offset = offset as usize;
                }
            }
            SeekFrom::End(offset) => {
                let t = len as i64 + offset;
                if t < 0 {
                    self.offset = 0;
                } else if t > len as i64 {
                    self.offset = len;
                } else {
                    self.offset = t as usize;
                }
            }
            SeekFrom::Current(offset) => {
                let t = self.offset as i64 + offset;
                if t < 0 {
                    self.offset = 0;
                } else if t > len as i64 {
                    self.offset = len;
                } else {
                    self.offset = t as usize;
                }
            }
        }

        Ok(self.offset as u64)
    }
}
