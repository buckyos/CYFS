use std::io::{SeekFrom};
use std::ops::{Deref, DerefMut};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use crate::{Chunk, ChunkMeta, ChunkMut};

pub struct MemChunk {
    buf: Vec<u8>,
    pos: usize,
    data_len: usize,
}

impl MemChunk {
    pub fn new(capacity: usize, data_len: usize) -> Self {
        let mut buf = Vec::new();
        buf.resize(capacity, 0);
        Self {
            buf,
            pos: 0,
            data_len
        }
    }

    pub fn resize(&mut self, len: usize) {
        if len < self.data_len {
            self.data_len = len;
        }
        self.buf.resize(len, 0);
    }

    fn read_inner(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        if self.pos >= self.data_len {
            Ok(0)
        } else if buf.len() > self.data_len - self.pos {
            unsafe {std::ptr::copy::<u8>(self.buf[self.pos..].as_ptr(), buf.as_mut_ptr(), self.data_len - self.pos)};
            let read_len = self.data_len - self.pos;
            self.pos = self.data_len;
            Ok(read_len)
        } else {
            unsafe {std::ptr::copy::<u8>(self.buf[self.pos..self.pos + buf.len()].as_ptr(), buf.as_mut_ptr(), buf.len())};
            let read_len = buf.len();
            self.pos = self.pos + read_len;
            Ok(read_len)
        }
    }

    fn seek_inner(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        match pos {
            SeekFrom::Start(pos) => {
                self.pos = pos as usize;
                Ok(pos)
            },
            SeekFrom::End(pos) => {
                if self.data_len as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                self.pos = (self.data_len as i64 + pos) as usize;
                Ok(self.pos as u64)
            },
            SeekFrom::Current(pos) => {
                if self.pos as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                self.pos = (self.pos as i64 + pos) as usize;
                Ok(self.pos as u64)
            }
        }
    }

    fn write_inner(&mut self, buf: &[u8]) -> BuckyResult<usize> {
        unsafe {
            if self.pos + buf.len() >= self.buf.len() {
                let write_size = self.buf.len() - self.pos;
                std::ptr::copy(buf.as_ptr(), self.buf.as_mut_ptr(), write_size);
                self.pos = self.buf.len();
                if self.pos > self.data_len {
                    self.data_len = self.pos;
                }
                Ok(write_size)
            } else {
                std::ptr::copy(buf.as_ptr(), self.buf.as_mut_ptr(), buf.len());
                self.pos += buf.len();
                if self.pos > self.data_len {
                    self.data_len = self.pos;
                }
                Ok(buf.len())
            }
        }
    }
}

impl From<Vec<u8>> for MemChunk {
    fn from(buf: Vec<u8>) -> Self {
        Self {
            data_len: buf.len(),
            buf,
            pos: 0
        }
    }
}

impl Deref for MemChunk {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.buf.as_slice()
    }
}

#[async_trait::async_trait]
impl Chunk for MemChunk {
    fn get_chunk_meta(&self) -> ChunkMeta {
        ChunkMeta::MemChunk(self.buf.clone())
    }

    fn get_len(&self) -> usize {
        self.buf.len()
    }

    fn into_vec(self: Box<Self>) -> Vec<u8> {
        self.buf
    }

    async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        self.read_inner(buf)
    }

    async fn seek(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        self.seek_inner(pos)
    }
}

impl DerefMut for MemChunk {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buf.as_mut_slice()
    }
}

#[async_trait::async_trait]
impl ChunkMut for MemChunk {
    async fn reset(&mut self) -> BuckyResult<()> {
        self.pos = 0;
        self.data_len = 0;
        Ok(())
    }

    async fn write(&mut self, buf: &[u8]) -> BuckyResult<usize> {
        self.write_inner(buf)
    }

    async fn flush(&mut self) -> BuckyResult<()> {
        Ok(())
    }
}

pub struct MemRefChunk<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> MemRefChunk<'a> {
    fn read_inner(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        if self.pos >= self.buf.len() {
            Ok(0)
        } else if buf.len() > self.buf.len() - self.pos {
            unsafe {std::ptr::copy::<u8>(self.buf[self.pos..].as_ptr(), buf.as_mut_ptr(), self.buf.len() - self.pos)};
            let read_len = self.buf.len() - self.pos;
            self.pos = self.buf.len();
            Ok(read_len)
        } else {
            unsafe {std::ptr::copy::<u8>(self.buf[self.pos..self.pos + buf.len()].as_ptr(), buf.as_mut_ptr(), buf.len())};
            let read_len = buf.len();
            self.pos = self.pos + read_len;
            Ok(read_len)
        }
    }

    fn seek_inner(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        match pos {
            SeekFrom::Start(pos) => {
                self.pos = pos as usize;
                Ok(pos)
            },
            SeekFrom::End(pos) => {
                if self.buf.len() as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                self.pos = (self.buf.len() as i64 + pos) as usize;
                Ok(self.pos as u64)
            },
            SeekFrom::Current(pos) => {
                if self.pos as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                self.pos = (self.pos as i64 + pos) as usize;
                Ok(self.pos as u64)
            }
        }
    }
}
impl <'a> From<&'a [u8]> for MemRefChunk<'a> {
    fn from(buf: &'a [u8]) -> Self {
        Self {
            buf,
            pos: 0
        }
    }
}

impl <'a> Deref for MemRefChunk<'a> {
    type Target = [u8];

    fn deref(&self) -> &'a Self::Target {
        self.buf
    }
}

#[async_trait::async_trait]
impl <'a> Chunk for MemRefChunk<'a> {
    fn get_chunk_meta(&self) -> ChunkMeta {
        ChunkMeta::MemChunk(self.buf.to_vec())
    }

    fn get_len(&self) -> usize {
        self.buf.len()
    }

    fn into_vec(self: Box<Self>) -> Vec<u8> {
        self.buf.to_vec()
    }

    async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        self.read_inner(buf)
    }

    async fn seek(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        self.seek_inner(pos)
    }
}

#[cfg(test)]
mod test_mem_chunk {
    use std::io::Write;
    use std::sync::Arc;
    use crate::{Chunk, ChunkMut, MemChunk, MemRefChunk, SharedMemChunk};

    #[test]
    fn test_mem_ref_chunk() {
        let buf = {
            let mut buf = Vec::<u8>::new();
            buf.resize(20, 0);
            Arc::new(buf)
        };
        let chunk = MemRefChunk::from(buf.as_slice());

        let s = &chunk[..];
        assert_eq!(s.len(), 20);
    }

    #[test]
    fn test_mem_async_test() {
        async_std::task::block_on(async move {
            let mut mem_chunk = MemChunk::new(20);
            mem_chunk.write("test".as_bytes()).await;
        })
    }

    #[test]
    fn test_share_mem_test() {
        async_std::task::block_on(async move {
            let mut mem_chunk = SharedMemChunk::new(20, "test").unwrap();
            mem_chunk.write("test".as_bytes()).await;

            let mut mem_chunk2 = SharedMemChunk::new(20, "test").unwrap();
            let mut buf = [0u8;4];
            mem_chunk2.read(&mut buf).await.unwrap();
            println!("{} {}", mem_chunk.len(), mem_chunk2.len());

            assert_eq!(20, mem_chunk.len());
            assert_eq!(20, mem_chunk2.len());
            assert_eq!("test", String::from_utf8_lossy(&buf).to_string().as_str());
        })
    }
}
