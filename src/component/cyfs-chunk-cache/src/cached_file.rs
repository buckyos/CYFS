use std::collections::HashMap;
use std::io::{SeekFrom};
use std::sync::Arc;
use cyfs_chunk_lib::{Chunk};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ChunkId, ChunkList, File, NamedObject};
use crate::{ChunkManager, ChunkType};

pub trait CYFSFile {

}

pub struct CachedFile {
    file: File,
    pos: u64,
    chunk_map: HashMap<ChunkId, Box<dyn Chunk>>,
    chunk_manager: Arc<ChunkManager>,
}

impl CachedFile {
    pub async fn open(file: File, chunk_manager: Arc<ChunkManager>) -> BuckyResult<Self> {
        Ok(Self {
            file,
            pos: 0,
            chunk_map: Default::default(),
            chunk_manager
        })
    }

    fn calc_chunk_by_pos(list: &[ChunkId], pos: u64) -> BuckyResult<(u64, ChunkId)> {
        let mut cur_pos = 0;
        for item in list.iter() {
            if cur_pos <= pos && cur_pos + item.len() as u64 > pos {
                return Ok((cur_pos, item.clone()));
            } else {
                cur_pos += item.len() as u64;
            }
        }

        let msg = format!("can't find chunkid by pos: {}", pos);
        log::warn!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
    }

    fn get_chunk_id_by_pos(&self, pos: u64) -> BuckyResult<(u64, ChunkId)> {
        let chunk_list = self.file.body().as_ref().unwrap().content().chunk_list();
        match chunk_list {
            ChunkList::ChunkInList(list) => {
                Self::calc_chunk_by_pos(list, pos)
            },
            ChunkList::ChunkInBundle(bundle) => {
                Self::calc_chunk_by_pos(bundle.chunk_list(), pos)
            },
            ChunkList::ChunkInFile(_) => {
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, "unsupport file type"))
            }
        }
    }

    async fn get_chunk_by_pos(&mut self, pos: u64) -> BuckyResult<(u64, ChunkId, Box<dyn Chunk>)> {
        let (chunk_pos, chunk_id) = self.get_chunk_id_by_pos(pos)?;
        if !self.chunk_map.contains_key(&chunk_id) {
            let chunk = self.chunk_manager.get_chunk(&chunk_id, ChunkType::MMapChunk).await?;
            self.chunk_map.insert(chunk_id.clone(), chunk);
        }
        Ok((chunk_pos, chunk_id.clone(), self.chunk_map.remove(&chunk_id).unwrap()))
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
        let mut tmp_buf = buf;
        if tmp_buf.len() == 0 {
            return Ok(0);
        }
        let mut read_len = 0;
        if self.pos >= self.file.desc().content().len() {
            return Ok(0);
        }
        loop {
            let (chunk_pos, chunk_id, mut chunk) = self.get_chunk_by_pos(self.pos).await?;

            let chunk_offset = self.pos - chunk_pos;
            chunk.seek(SeekFrom::Start(chunk_offset)).await?;
            let read_size = chunk.read(tmp_buf).await?;
            if read_size == 0 {
                let msg = format!("read 0 bytes from chunk {}", chunk_id.to_string());
                log::error!("{}", msg.as_str());
                return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
            }
            tmp_buf = &mut tmp_buf[read_size..];
            self.pos += read_size as u64;
            read_len += read_size;
            self.chunk_map.insert(chunk_id, chunk);

            if tmp_buf.len() == 0 || self.pos >= self.file.desc().content().len() {
                break;
            }
        }

        Ok(read_len)
    }

    pub async fn seek(&mut self, pos: SeekFrom) -> BuckyResult<u64> {
        let this = self;
        match pos {
            SeekFrom::Start(pos) => {
                this.pos = pos;
                Ok(pos)
            },
            SeekFrom::End(pos) => {
                if this.file.desc().content().len() as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.pos = (this.file.desc().content().len() as i64 + pos) as u64;
                Ok(this.pos as u64)
            },
            SeekFrom::Current(pos) => {
                if this.pos as i64 + pos < 0 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("seek failed")));
                }
                this.pos = (this.pos as i64 + pos) as u64;
                Ok(this.pos)
            }
        }
    }
}
