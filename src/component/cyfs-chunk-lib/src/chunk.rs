#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use crate::SharedMemChunk;
use crate::{MMapChunk, MemChunk};
use cyfs_base::*;
use std::future::Future;
use std::io::SeekFrom;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};

pub const CHUNK_SIZE: u64 = 4 * 1024 * 1024;

#[derive(RawEncode, RawDecode)]
pub enum ChunkMeta {
    MMapChunk(String, Option<u32>),
    SharedMemChunk(String, u32, u32),
    MemChunk(Vec<u8>),
}

impl ChunkMeta {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub async fn to_chunk(self) -> BuckyResult<Box<dyn Chunk>> {
        match self {
            ChunkMeta::SharedMemChunk(share_id, capacity, data_len) => Ok(Box::new(
                SharedMemChunk::new(capacity as usize, data_len as usize, share_id.as_str())?,
            )),
            ChunkMeta::MMapChunk(mmap_id, len) => {
                Ok(Box::new(MMapChunk::open(mmap_id, len).await?))
            }
            ChunkMeta::MemChunk(data) => Ok(Box::new(MemChunk::from(data))),
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    pub async fn to_chunk(self) -> BuckyResult<Box<dyn Chunk>> {
        match self {
            ChunkMeta::SharedMemChunk(share_id, capacity, data_len) => {
                let msg = format!("unsupport share mem chunk in android or ios");
                log::error!("{}", msg.as_str());
                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
            ChunkMeta::MMapChunk(mmap_id, len) => {
                Ok(Box::new(MMapChunk::open(mmap_id, len).await?))
            }
            ChunkMeta::MemChunk(data) => Ok(Box::new(MemChunk::from(data))),
        }
    }
}

#[async_trait::async_trait]
pub trait Chunk: Deref<Target = [u8]> + Send + Sync {
    fn calculate_id(&self) -> ChunkId {
        let hash = hash_data(&self[..self.get_len() as usize]);
        ChunkId::new(&hash, self.get_len() as u32)
    }

    fn get_chunk_meta(&self) -> ChunkMeta;
    fn get_len(&self) -> usize;
    fn into_vec(self: Box<Self>) -> Vec<u8>;

    async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize>;
    async fn seek(&mut self, pos: SeekFrom) -> BuckyResult<u64>;
}

pub struct ChunkRead {
    chunk: Box<dyn Chunk>,
    read_future: Mutex<Option<Pin<Box<dyn Future<Output = BuckyResult<usize>> + Send>>>>,
    seek_future: Mutex<Option<Pin<Box<dyn Future<Output = BuckyResult<u64>> + Send>>>>,
}

impl ChunkRead {
    pub fn new(chunk: Box<dyn Chunk>) -> Self {
        Self {
            chunk,
            read_future: Mutex::new(None),
            seek_future: Mutex::new(None),
        }
    }
}

impl async_std::io::Read for ChunkRead {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        unsafe {
            let this: &'static mut Self = std::mem::transmute(self.get_unchecked_mut());
            let buf: &'static mut [u8] = std::mem::transmute(buf);
            let mut future = this.read_future.lock().unwrap();
            if future.is_none() {
                *future = Some(Box::pin(this.chunk.read(buf)));
            }
            match future.as_mut().unwrap().as_mut().poll(cx) {
                Poll::Ready(ret) => {
                    *future = None;
                    match ret {
                        Ok(ret) => Poll::Ready(Ok(ret)),
                        Err(e) => Poll::Ready(Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("{}", e),
                        ))),
                    }
                }
                Poll::Pending => Poll::Pending,
            }
        }
    }
}

impl async_std::io::Seek for ChunkRead {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<std::io::Result<u64>> {
        unsafe {
            let this: &'static mut Self = std::mem::transmute(self.get_unchecked_mut());
            let mut future = this.seek_future.lock().unwrap();
            if future.is_none() {
                *future = Some(Box::pin(this.chunk.seek(pos)));
            }
            match future.as_mut().unwrap().as_mut().poll(cx) {
                Poll::Ready(ret) => {
                    *future = None;
                    match ret {
                        Ok(ret) => Poll::Ready(Ok(ret)),
                        Err(e) => Poll::Ready(Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("{}", e),
                        ))),
                    }
                }
                Poll::Pending => Poll::Pending,
            }
        }
    }
}

use async_std::io::Seek;
use std::ops::Range;

pub struct ChunkReadWithRanges {
    reader: ChunkRead,
    ranges: Vec<Range<u64>>,
}

impl ChunkReadWithRanges {
    pub fn new(reader: ChunkRead, ranges: Vec<Range<u64>>) -> Self {
        Self { reader, ranges }
    }
}

impl async_std::io::Read for ChunkReadWithRanges {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        loop {
            if self.ranges.len() == 0 {
                break Poll::Ready(Ok(0));
            }

            let mut range = self.as_ref().ranges[0].clone();
            if range.is_empty() {
                self.ranges.remove(0);
                continue;
            }

            // seek with the current range
            match Pin::new(&mut self.reader).poll_seek(cx, SeekFrom::Start(range.start)) {
                Poll::Ready(ret) => {
                    match ret {
                        Ok(pos) => {
                            if pos != range.start {
                                let msg = format!("poll seek with range but ret pos not match! range={:?}, pos={}", range, pos);
                                log::error!("{}", msg);

                                break Poll::Ready(Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    msg,
                                )));
                            }
                        }
                        Err(e) => {
                            break Poll::Ready(Err(e));
                        }
                    }
                }
                Poll::Pending => {
                    break Poll::Pending;
                }
            }

            // read max bytes as range_len
            let range_len = (range.end - range.start) as usize;
            let range_buf = if buf.len() > range_len {
                &mut buf[..range_len]
            } else {
                buf
            };

            match Pin::new(&mut self.reader).poll_read(cx, range_buf) {
                Poll::Ready(ret) => match ret {
                    Ok(mut size) => {
                        assert!(size <= range_len);

                        if size > range_len {
                            size = range_len;
                        }

                        range.start += size as u64;
                        if range.is_empty() {
                            // current range is completed
                            self.ranges.remove(0);
                        } else {
                            // current range updated
                            self.ranges[0] = range;
                        }

                        break Poll::Ready(Ok(size));
                    }
                    Err(e) => {
                        break Poll::Ready(Err(e));
                    }
                },
                Poll::Pending => {
                    break Poll::Pending;
                }
            }
        }
    }
}

#[async_trait::async_trait]
pub trait ChunkMut: Chunk {
    async fn reset(&mut self) -> BuckyResult<()>;
    async fn write(&mut self, buf: &[u8]) -> BuckyResult<usize>;
    async fn flush(&mut self) -> BuckyResult<()>;
}

pub struct ChunkWrite {
    chunk: Box<dyn ChunkMut>,
    future: Mutex<Option<Pin<Box<dyn Future<Output = BuckyResult<usize>>>>>>,
    flush_future: Mutex<Option<Pin<Box<dyn Future<Output = BuckyResult<()>>>>>>,
}

impl ChunkWrite {
    pub fn new(chunk: Box<dyn ChunkMut>) -> Self {
        Self {
            chunk,
            future: Mutex::new(None),
            flush_future: Mutex::new(None),
        }
    }
}

impl async_std::io::Write for ChunkWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        unsafe {
            let this: &'static mut Self = std::mem::transmute(self.get_unchecked_mut());
            let buf: &'static [u8] = std::mem::transmute(buf);
            let mut future = this.future.lock().unwrap();
            if future.is_none() {
                *future = Some(Box::pin(this.chunk.write(buf)));
            }
            match future.as_mut().unwrap().as_mut().poll(cx) {
                Poll::Ready(ret) => {
                    *future = None;
                    match ret {
                        Ok(ret) => Poll::Ready(Ok(ret)),
                        Err(e) => Poll::Ready(Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("{}", e),
                        ))),
                    }
                }
                Poll::Pending => Poll::Pending,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        unsafe {
            let this: &'static mut Self = std::mem::transmute(self.get_unchecked_mut());
            let mut flush_future = this.flush_future.lock().unwrap();
            if flush_future.is_none() {
                *flush_future = Some(Box::pin(this.chunk.flush()));
            }
            match flush_future.as_mut().unwrap().as_mut().poll(cx) {
                Poll::Ready(ret) => {
                    *flush_future = None;
                    match ret {
                        Ok(ret) => Poll::Ready(Ok(ret)),
                        Err(e) => Poll::Ready(Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("{}", e),
                        ))),
                    }
                }
                Poll::Pending => Poll::Pending,
            }
        }
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
