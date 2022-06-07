use std::fs::{File, OpenOptions};
use std::io::{SeekFrom};
use std::ops::{Deref, DerefMut};
use std::path::Path;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use memmap2::{MmapMut, MmapOptions};
use crate::{Chunk, ChunkMeta, ChunkMut};

pub struct MMapChunk {
    file_path: String,
    mmap: memmap2::Mmap,
    read_pos: usize,
    data_len: usize,
}

impl MMapChunk {
    pub async fn open<P: AsRef<Path>>(path: P, data_len: Option<u32>) -> BuckyResult<Self> {
        let file_path = path.as_ref().to_string_lossy().to_string();
        log::info!("MMapChunk open {}", file_path.as_str());
        let tmp_path = path.as_ref().to_path_buf();
        let ret: BuckyResult<Self> = async_std::task::spawn_blocking(move || {
            unsafe {
                let file = File::open(tmp_path.as_path()).map_err(|e| {
                    let msg = format!("[{}:{}] open {} failed.err {}", file!(), line!(), tmp_path.to_string_lossy().to_string(), e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;
                let mmap = MmapOptions::new().map(&file).map_err(|e| {
                    let msg = format!("[{}:{}] create file {} map failed.err {}", file!(), line!(), tmp_path.to_string_lossy().to_string(), e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;
                let data_len = if data_len.is_some() {
                    if data_len.unwrap() as usize > mmap.len() {
                        mmap.len()
                    } else {
                        data_len.unwrap() as usize
                    }
                } else {
                    mmap.len()
                };
                Ok(Self {
                    file_path,
                    mmap,
                    read_pos: 0,
                    data_len
                })
            }
        }).await;
        log::info!("MMapChunk open {} success", path.as_ref().to_string_lossy().to_string());
        ret
    }
}

impl Deref for MMapChunk {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.mmap
    }
}

#[async_trait::async_trait]
impl Chunk for MMapChunk {
    fn get_chunk_meta(&self) -> ChunkMeta {
        ChunkMeta::MMapChunk(self.file_path.clone(), Some(self.data_len as u32))
    }

    fn get_len(&self) -> usize {
        self.mmap.len()
    }

    fn into_vec(self: Box<Self>) -> Vec<u8> {
        self.mmap.to_vec()
    }

    async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        let this = self;
        if this.read_pos >= this.data_len {
            Ok(0)
        } else if buf.len() > this.data_len - this.read_pos {
            unsafe {std::ptr::copy::<u8>(this.mmap[this.read_pos..].as_ptr(), buf.as_mut_ptr(), this.data_len - this.read_pos)};
            let read_len = this.data_len - this.read_pos;
            this.read_pos = this.data_len;
            Ok(read_len)
        } else {
            unsafe {std::ptr::copy::<u8>(this.mmap[this.read_pos..this.read_pos + buf.len()].as_ptr(), buf.as_mut_ptr(), buf.len())};
            let read_len = buf.len();
            this.read_pos = this.read_pos + read_len;
            Ok(read_len)
        }
    }

    async fn seek(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        let this = self;
        match pos {
            SeekFrom::Start(pos) => {
                this.read_pos = pos as usize;
                Ok(pos)
            },
            SeekFrom::End(pos) => {
                if this.data_len as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.read_pos = (this.data_len as i64 + pos) as usize;
                Ok(this.read_pos as u64)
            },
            SeekFrom::Current(pos) => {
                if this.read_pos as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.read_pos = (this.read_pos as i64 + pos) as usize;
                Ok(this.read_pos as u64)
            }
        }
    }
}

pub struct MMapChunkMut {
    file_path: String,
    mmap: memmap2::MmapMut,
    cur_pos: usize,
    data_len: usize
}

impl MMapChunkMut {
    pub async fn open<P: AsRef<Path>>(path: P, capacity: u64, data_len: Option<u64>) -> BuckyResult<Self> {
        let file_path = path.as_ref().to_string_lossy().to_string();
        let path = path.as_ref().to_path_buf();
        async_std::task::spawn_blocking(move || {
            unsafe {
                let file = OpenOptions::new().read(true).write(true).create(true).open(path.as_path()).map_err(|e| {
                    let msg = format!("[{}:{}] open {} failed.err {}", file!(), line!(), path.to_string_lossy().to_string(), e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;

                let data_len = if data_len.is_some() {
                    if data_len.unwrap() > capacity {
                        capacity
                    } else {
                        data_len.unwrap()
                    }
                } else {
                    let mut data_len = file.metadata().map_err(|e| {
                        let msg = format!("[{}:{}] get {} meta failed.err {}", file!(), line!(), path.to_string_lossy().to_string(), e);
                        log::error!("{}", msg.as_str());
                        BuckyError::new(BuckyErrorCode::Failed, msg)
                    })?.len();
                    if data_len > capacity {
                        data_len = capacity;
                    }
                    data_len
                };
                file.set_len(capacity).map_err(|e| {
                    let msg = format!("[{}:{}] set file {}  len {} failed.err {}", file!(), line!(), path.to_string_lossy().to_string(), capacity, e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;
                let mmap = MmapMut::map_mut(&file).map_err(|e| {
                    let msg = format!("[{}:{}] create file {} map failed.err {}", file!(), line!(), path.to_string_lossy().to_string(), e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;

                Ok(Self {
                    file_path,
                    mmap,
                    cur_pos: 0,
                    data_len: data_len as usize
                })
            }
        }).await
    }
}

impl Deref for MMapChunkMut {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.mmap
    }
}

impl DerefMut for MMapChunkMut {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mmap
    }
}

#[async_trait::async_trait]
impl Chunk for MMapChunkMut {
    fn get_chunk_meta(&self) -> ChunkMeta {
        ChunkMeta::MMapChunk(self.file_path.clone(), Some(self.data_len as u32))
    }

    fn get_len(&self) -> usize {
        self.data_len
    }

    fn into_vec(self: Box<Self>) -> Vec<u8> {
        self.mmap[..self.data_len].to_vec()
    }

    async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        let this = self;
        if this.cur_pos >= this.data_len {
            Ok(0)
        } else if buf.len() > this.data_len - this.cur_pos {
            unsafe {std::ptr::copy::<u8>(this.mmap[this.cur_pos..].as_ptr(), buf.as_mut_ptr(), this.data_len - this.cur_pos)};
            let read_len = this.data_len - this.cur_pos;
            this.cur_pos = this.data_len;
            Ok(read_len)
        } else {
            unsafe {std::ptr::copy::<u8>(this.mmap[this.cur_pos..this.cur_pos + buf.len()].as_ptr(), buf.as_mut_ptr(), buf.len())};
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

#[async_trait::async_trait]
impl ChunkMut for MMapChunkMut {
    async fn reset(&mut self) -> BuckyResult<()> {
        self.cur_pos = 0;
        self.data_len = 0;
        Ok(())
    }

    async fn write(&mut self, buf: &[u8]) -> BuckyResult<usize> {
        let this = self;
        unsafe {
            if this.cur_pos + buf.len() >= this.mmap.len() {
                let write_size = this.mmap.len() - this.cur_pos;
                std::ptr::copy(buf.as_ptr(), this.mmap[this.cur_pos..].as_mut_ptr(), write_size);
                this.cur_pos = this.mmap.len();
                if this.cur_pos > this.data_len {
                    this.data_len = this.cur_pos;
                }
                Ok(write_size)
            } else {
                std::ptr::copy(buf.as_ptr(), this.mmap[this.cur_pos..].as_mut_ptr(), buf.len());
                this.cur_pos += buf.len();
                if this.cur_pos > this.data_len {
                    this.data_len = this.cur_pos;
                }
                Ok(buf.len())
            }
        }
    }

    async fn flush(&mut self) -> BuckyResult<()> {
        self.mmap.flush().map_err(|e| {
            let msg = format!("flush err {}", e);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })
    }
}
