use std::io::SeekFrom;
use std::ops::{Deref, DerefMut};
use shared_memory::{Shmem, ShmemConf, ShmemError};
use cyfs_base::*;
use crate::{Chunk, ChunkMeta, ChunkMut};

pub struct SharedMemChunk {
    unique_id: String,
    shmem: Shmem,
    cur_pos: usize,
    capacity: usize,
    data_len: usize,
}

unsafe impl Sync for SharedMemChunk {

}

unsafe impl Send for SharedMemChunk {

}

impl SharedMemChunk {
    pub fn new(capacity: usize, data_len: usize, unique_id: &str) -> BuckyResult<Self> {
        match ShmemConf::new().size(capacity).os_id(unique_id).create() {
            Ok(shmem) => Ok(Self {
                unique_id: unique_id.to_string(),
                shmem,
                cur_pos: 0,
                capacity,
                data_len
            }),
            Err(ShmemError::MappingIdExists) => {
                match ShmemConf::new().size(capacity).os_id(unique_id).open() {
                    Ok(shmem) => Ok(Self {
                        unique_id: unique_id.to_string(),
                        shmem,
                        cur_pos: 0,
                        capacity,
                        data_len
                    }),
                    Err(e) => {
                        let msg = format!("open shared mem {} failed. {}", unique_id, e);
                        log::error!("{}", msg.as_str());
                        Err(BuckyError::new(BuckyErrorCode::Failed, msg))
                    }
                }
            },
            Err(e) => {
                let msg = format!("create shared mem {} failed. {}", unique_id, e);
                log::error!("{}", msg.as_str());
                Err(BuckyError::new(BuckyErrorCode::Failed, msg))
            }
        }
    }
}

#[async_trait::async_trait]
impl Chunk for SharedMemChunk {
    fn get_chunk_meta(&self) -> ChunkMeta {
        ChunkMeta::SharedMemChunk(self.unique_id.clone(), self.capacity as u32, self.data_len as u32)
    }

    fn get_len(&self) -> usize {
        self.data_len
    }

    fn into_vec(self: Box<Self>) -> Vec<u8> {
        unsafe {
            self.shmem.as_slice().to_vec()
        }
    }

    async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        let this = self;
        let shmem = &this.shmem;
        if this.cur_pos >= this.data_len {
            Ok(0)
        } else if buf.len() > this.data_len - this.cur_pos {
            unsafe {std::ptr::copy::<u8>(shmem.as_slice()[this.cur_pos..].as_ptr(), buf.as_mut_ptr(), this.data_len - this.cur_pos)};
            let read_len = shmem.len() - this.cur_pos;
            this.cur_pos = shmem.len();
            Ok(read_len)
        } else {
            unsafe {std::ptr::copy::<u8>(shmem.as_slice()[this.cur_pos..this.cur_pos + buf.len()].as_ptr(), buf.as_mut_ptr(), buf.len())};
            let read_len = buf.len();
            this.cur_pos = this.cur_pos + read_len;
            Ok(read_len)
        }
    }

    async fn seek(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        let this = self;
        match pos {
            SeekFrom::Start(pos) => {
                this.cur_pos = pos as usize;
                Ok(pos)
            },
            SeekFrom::End(pos) => {
                if this.data_len as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.cur_pos = (this.data_len as i64 + pos) as usize;
                Ok(this.cur_pos as u64)
            },
            SeekFrom::Current(pos) => {
                if this.cur_pos as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.cur_pos = (this.cur_pos as i64 + pos) as usize;
                Ok(this.cur_pos as u64)
            }
        }
    }
}

impl Deref for SharedMemChunk {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe {
            &self.shmem.as_slice()[..self.capacity]
        }
    }
}

#[async_trait::async_trait]
impl ChunkMut for SharedMemChunk {
    async fn reset(&mut self) -> BuckyResult<()> {
        self.cur_pos = 0;
        self.data_len = 0;
        Ok(())
    }

    async fn write(&mut self, buf: &[u8]) -> BuckyResult<usize> {
        let this = self;
        let shmem = &this.shmem;
        unsafe {
            if this.cur_pos + buf.len() >= shmem.len() {
                let write_size = shmem.len() - this.cur_pos;
                std::ptr::copy(buf.as_ptr(), shmem.as_ptr().offset(this.cur_pos as isize), write_size);
                this.cur_pos = shmem.len();
                if this.cur_pos > this.data_len {
                    this.data_len = this.cur_pos;
                }
                Ok(write_size)
            } else {
                std::ptr::copy(buf.as_ptr(), shmem.as_ptr().offset(this.cur_pos as isize), buf.len());
                this.cur_pos += buf.len();
                if this.cur_pos > this.data_len {
                    this.data_len = this.cur_pos;
                }
                Ok(buf.len())
            }
        }
    }

    async fn flush(&mut self) -> BuckyResult<()> {
        Ok(())
    }
}

#[cfg(not(target_os = "android"))]
impl DerefMut for SharedMemChunk {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut self.shmem.as_slice_mut()[..self.capacity]
        }
    }
}
