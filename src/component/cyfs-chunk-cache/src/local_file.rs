use std::collections::HashMap;
use std::io::{SeekFrom};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ChunkId, File, NamedObject};
use memmap2::MmapMut;
use cyfs_chunk_lib::{Chunk, ChunkMeta, ChunkMut};

pub struct LocalFileChunk<'a> {
    chunk_pos: u64,
    chunk_len: u64,
    mmap: &'a MmapMut,
    cur_pos: u64,
}

impl <'a> LocalFileChunk<'a> {
    pub fn new(chunk_pos: u64, chunk_len: u64, mmap: &'a MmapMut) -> Self {
        Self {
            chunk_pos,
            chunk_len,
            mmap,
            cur_pos: 0
        }
    }

    pub fn get_chunk_range(&self) -> (u64, u64) {
        (self.chunk_pos, self.chunk_len)
    }
}

impl <'a> Deref for LocalFileChunk<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.mmap[self.chunk_pos as usize..(self.chunk_pos + self.chunk_len) as usize]
    }
}

#[async_trait::async_trait]
impl <'a> Chunk for LocalFileChunk<'a> {
    fn get_chunk_meta(&self) -> ChunkMeta {
        ChunkMeta::MemChunk((&self[..]).to_vec())
    }

    fn get_len(&self) -> usize {
        self.chunk_len as usize
    }

    fn into_vec(self: Box<Self>) -> Vec<u8> {
        (&self[..]).to_vec()
    }

    async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        let this = self;
        if this.cur_pos >= this.chunk_len {
            Ok(0)
        } else if buf.len() > (this.chunk_len - this.cur_pos) as usize {
            unsafe { std::ptr::copy::<u8>(this.mmap[(this.cur_pos + this.chunk_pos) as usize..].as_ptr(), buf.as_mut_ptr(), (this.chunk_len - this.cur_pos) as usize) };
            let read_len = this.chunk_len - this.cur_pos;
            this.cur_pos = this.chunk_len;
            Ok(read_len as usize)
        } else {
            unsafe { std::ptr::copy::<u8>(this.mmap[(this.cur_pos + this.chunk_pos) as usize..(this.cur_pos + this.chunk_pos) as usize + buf.len()].as_ptr(), buf.as_mut_ptr(), buf.len()) };
            let read_len = buf.len();
            this.cur_pos = this.cur_pos + read_len as u64;
            Ok(read_len)
        }
    }

    async fn seek(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        let this = self;
        match pos {
            SeekFrom::Start(pos) => {
                this.cur_pos = pos;
                Ok(pos)
            },
            SeekFrom::End(pos) => {
                if this.chunk_len as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.cur_pos = (this.chunk_len as i64 + pos) as u64;
                Ok(this.cur_pos)
            },
            SeekFrom::Current(pos) => {
                if this.cur_pos as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.cur_pos = (this.cur_pos as i64 + pos) as u64;
                Ok(this.cur_pos)
            }
        }
    }
}

pub struct LocalFileChunkMut<'a> {
    chunk_pos: u64,
    chunk_len: u64,
    mmap: &'a mut MmapMut,
    cur_pos: u64,
}

impl <'a> LocalFileChunkMut<'a> {
    pub fn new(chunk_pos: u64, chunk_len: u64, mmap: &'a mut MmapMut) -> Self {
        Self {
            chunk_pos,
            chunk_len,
            mmap,
            cur_pos: 0
        }
    }
}

impl <'a> Deref for LocalFileChunkMut<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.mmap[self.chunk_pos as usize..(self.chunk_pos + self.chunk_len) as usize]
    }
}

impl <'a> DerefMut for LocalFileChunkMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mmap[self.chunk_pos as usize..(self.chunk_pos + self.chunk_len) as usize]
    }
}

#[async_trait::async_trait]
impl<'a> ChunkMut for LocalFileChunkMut<'a> {
    async fn reset(&mut self) -> BuckyResult<()> {
        self.cur_pos = 0;
        Ok(())
    }

    async fn write(&mut self, buf: &[u8]) -> BuckyResult<usize> {
        let this = self;
        unsafe {
            if this.cur_pos as usize + buf.len() >= this.chunk_len as usize {
                let cur_pos = this.cur_pos;
                std::ptr::copy(buf.as_ptr(), this.deref_mut()[cur_pos as usize..].as_mut_ptr(), (this.chunk_len - this.cur_pos) as usize);
                this.cur_pos = this.chunk_len;
                Ok((this.chunk_len - this.cur_pos) as usize)
            } else {
                let cur_pos = this.cur_pos;
                std::ptr::copy(buf.as_ptr(), this.deref_mut()[cur_pos as usize..].as_mut_ptr(), buf.len());
                this.cur_pos += buf.len() as u64;
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

#[async_trait::async_trait]
impl <'a> Chunk for LocalFileChunkMut<'a> {
    fn get_chunk_meta(&self) -> ChunkMeta {
        ChunkMeta::MemChunk((&self[..]).to_vec())
    }

    fn get_len(&self) -> usize {
        self.chunk_len as usize
    }

    fn into_vec(self: Box<Self>) -> Vec<u8> {
        (&self[..]).to_vec()
    }

    async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        let this = self;
        if this.cur_pos >= this.chunk_len {
            Ok(0)
        } else if buf.len() > (this.chunk_len - this.cur_pos) as usize {
            unsafe { std::ptr::copy::<u8>(this.mmap[(this.cur_pos + this.chunk_pos) as usize..].as_ptr(), buf.as_mut_ptr(), (this.chunk_len - this.cur_pos) as usize) };
            let read_len = this.chunk_len - this.cur_pos;
            this.cur_pos = this.chunk_len;
            Ok(read_len as usize)
        } else {
            unsafe { std::ptr::copy::<u8>(this.mmap[(this.cur_pos + this.chunk_pos) as usize..(this.cur_pos + this.chunk_pos) as usize + buf.len()].as_ptr(), buf.as_mut_ptr(), buf.len()) };
            let read_len = buf.len();
            this.cur_pos = this.cur_pos + read_len as u64;
            Ok(read_len)
        }
    }

    async fn seek(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        let this = self;
        match pos {
            SeekFrom::Start(pos) => {
                this.cur_pos = pos;
                Ok(pos)
            },
            SeekFrom::End(pos) => {
                if this.chunk_len as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.cur_pos = (this.chunk_len as i64 + pos) as u64;
                Ok(this.cur_pos)
            },
            SeekFrom::Current(pos) => {
                if this.cur_pos as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.cur_pos = (this.cur_pos as i64 + pos) as u64;
                Ok(this.cur_pos)
            }
        }
    }
}

pub struct LocalFile {
    file: File,
    chunk_map: HashMap<ChunkId, Vec<(u64, u64)>>,
    local_path: PathBuf,
    mmap: MmapMut,
}

impl LocalFile {
    pub async fn open(local_path: PathBuf, file: File) -> BuckyResult<Self> {
        async_std::task::spawn_blocking(move || {
            let file_handle = if !local_path.exists() {
                let file_handle = std::fs::OpenOptions::new().create(true).read(true).write(true).open(local_path.as_path()).map_err(|e| {
                    let msg = format!("open file {} failed.err {}", local_path.to_string_lossy().to_string(), e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;
                file_handle.set_len(file.desc().content().len()).map_err(|e| {
                    let msg = format!("[{}:{}] set file {}  len {} failed.err {}", file!(), line!(), local_path.to_string_lossy().to_string(), file.desc().content().len(), e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;
                file_handle
            } else {
                let file_handle = std::fs::OpenOptions::new().read(true).write(true).open(local_path.as_path()).map_err(|e| {
                    let msg = format!("open file {} failed.err {}", local_path.to_string_lossy().to_string(), e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;
                let meta = std::fs::metadata(local_path.as_path()).map_err(|e| {
                    let msg = format!("read file meta {} failed.err {}", local_path.to_string_lossy().to_string(), e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;
                if meta.len() != file.desc().content().len() {
                    file_handle.set_len(file.desc().content().len()).map_err(|e| {
                        let msg = format!("[{}:{}] set file {}  len {} failed.err {}", file!(), line!(), local_path.to_string_lossy().to_string(), file.desc().content().len(), e);
                        log::error!("{}", msg.as_str());
                        BuckyError::new(BuckyErrorCode::Failed, msg)
                    })?;
                }
                file_handle
            };

            let mmap = unsafe {MmapMut::map_mut(&file_handle).map_err(|e| {
                let msg = format!("[{}:{}] create file {} map failed.err {}", file!(), line!(), local_path.to_string_lossy().to_string(), e);
                log::error!("{}", msg.as_str());
                BuckyError::new(BuckyErrorCode::Failed, msg)
            })?};


            let mut chunk_map = HashMap::new();
            if let Some(chunk_list) = file.body().as_ref().unwrap().content().chunk_list().inner_chunk_list() {
                let mut pos = 0;
                for chunk_id in chunk_list.iter() {
                    if !chunk_map.contains_key(chunk_id) {
                        chunk_map.insert(chunk_id.clone(), vec![(pos as u64, chunk_id.len() as u64)]);
                    } else {
                        chunk_map.get_mut(chunk_id).unwrap().push((pos as u64, chunk_id.len() as u64));
                    }
                    pos += chunk_id.len();
                }
            }
            Ok(Self {
                file,
                chunk_map,
                local_path,
                mmap
            })
        }).await
    }

    pub async fn get_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<LocalFileChunk<'_>> {
        match self.chunk_map.get(chunk_id) {
            Some(range_list) => {
                Ok(LocalFileChunk::new(range_list[0].0, range_list[0].1, &self.mmap))
            },
            None => {
                Err(BuckyError::new(BuckyErrorCode::NotFound, format!("chunk {} not found in {}", chunk_id.to_string(), self.local_path.to_string_lossy().to_string())))
            }
        }
    }

    pub async fn get_chunk_range_list(&self, chunk_id: &ChunkId) -> BuckyResult<&Vec<(u64, u64)>> {
        match self.chunk_map.get(chunk_id) {
            Some(range_list) => {
                Ok(range_list)
            },
            None => {
                Err(BuckyError::new(BuckyErrorCode::NotFound, format!("chunk {} not found in {}", chunk_id.to_string(), self.local_path.to_string_lossy().to_string())))
            }
        }
    }

    pub async fn get_chunk_mut(&mut self, chunk_id: &ChunkId) -> BuckyResult<LocalFileChunkMut<'_>> {
        match self.chunk_map.get(chunk_id) {
            Some(range_list) => {
                Ok(LocalFileChunkMut::new(range_list[0].0, range_list[0].1, &mut self.mmap))
            },
            None => {
                Err(BuckyError::new(BuckyErrorCode::NotFound, format!("chunk {} not found in {}", chunk_id.to_string(), self.local_path.to_string_lossy().to_string())))
            }
        }
    }

    pub async fn put_chunk(&mut self, chunk_id: &ChunkId, chunk: &dyn Chunk) -> BuckyResult<()> {
        let this = self;
        let chunk_id = chunk_id.clone();
        match this.chunk_map.get(&chunk_id) {
            Some(chunk_list) => {
                for (chunk_pos, chunk_len) in chunk_list.iter() {
                    unsafe {
                        std::ptr::copy(chunk.deref().as_ptr(), this.mmap[*chunk_pos as usize..(*chunk_pos + *chunk_len) as usize].as_mut_ptr(), *chunk_len as usize);
                    }
                }
                Ok(())
            },
            None => {
                Err(BuckyError::new(BuckyErrorCode::NotFound, format!("chunk {} not found in {}", chunk_id.to_string(), this.local_path.to_string_lossy().to_string())))
            }
        }
    }

    pub async fn flush(&self) -> BuckyResult<()> {
        let this = self;
        this.mmap.flush().map_err(|e| {
            let msg = format!("flush err {}", e);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })
    }

    pub async fn is_exist(&self, chunk_id: &ChunkId) -> bool {
        match self.chunk_map.get(chunk_id) {
            Some(_) => true,
            None => false
        }
    }

    pub fn get_path(&self) -> &Path {
        self.local_path.as_path()
    }
}

impl Drop for LocalFile {
    fn drop(&mut self) {
        let _ = self.mmap.flush().map_err(|e| {
            let msg = format!("flush err {}", e);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
        });
    }
}

#[cfg(test)]
mod test_local_file {

}
