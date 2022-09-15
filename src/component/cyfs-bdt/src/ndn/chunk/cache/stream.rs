use std::{
    sync::{Arc, RwLock}, 
    ops::Range
};
use async_std::{
    task
};
use cyfs_base::*;
use crate::{
    types::*
};
use super::super::super::{
    types::*, 
    channel::{protocol::v0::*}
};
use super::super::{
    storage::*
};
use super::{
    encode::*
};

struct ReadyState {
    cache: Arc<Vec<u8>>, 
    index_queue: IndexQueue, 
}

enum StateImpl {
    Pending,
    Ready(ReadyState), 
    Err(BuckyErrorCode)
}

struct EncoderImpl {
    chunk: ChunkId,
    desc: ChunkEncodeDesc, 
    range_size: u16, 
    end_index: u32,  
    state: RwLock<StateImpl>
}

#[derive(Clone)]
pub struct StreamEncoder(Arc<EncoderImpl>);

impl std::fmt::Display for StreamEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamEncoder{{chunk:{},range_size:{}}}", self.chunk(), self.range_size())
    }
}


impl StreamEncoder {
    pub fn from_reader(
        reader: Arc<Box<dyn ChunkReader>>, 
        chunk: &ChunkId, 
        desc: &ChunkEncodeDesc, 
    ) -> Self {
        let range_size = PieceData::max_payload();
        let end_index = (chunk.len() + range_size - 1) / range_size - 1;

        let arc_self = Self(Arc::new(EncoderImpl {
            chunk: chunk.clone(), 
            desc: desc.clone(), 
            end_index: end_index as u32, 
            range_size: range_size as u16, 
            state: RwLock::new(StateImpl::Pending)
        }));
        trace!("{} begin load from store", arc_self);
        {
            let arc_self = arc_self.clone();
            let chunk = chunk.clone();
            task::spawn(async move {
                let ret = reader.get(&chunk).await;
                let state =  &mut *arc_self.0.state.write().unwrap();
                match &state {
                    StateImpl::Pending => {
                    }, 
                    _ => unreachable!()
                };
                match ret {
                    Ok(content) => {
                        info!("{} finish read", arc_self);
                        *state = StateImpl::Ready(ReadyState {
                            cache: content, 
                            index_queue: IndexQueue::new(0, arc_self.end_index())
                        });
                    }, 
                    Err(err) => {
                        error!("{} load chunk failed for {}", arc_self, err);
                        *state = StateImpl::Err(err.code());
                    }
                }
            });
        }
        arc_self
    } 

    pub fn end_index(&self) -> u32 {
        self.0.end_index
    }

    pub fn range_size(&self) -> u16 {
        self.0.range_size
    }
}

impl ChunkEncoder2 for StreamEncoder {
    fn clone_as_encoder(&self) -> Box<dyn ChunkEncoder2> {
        Box::new(self.clone())
    }

    fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    fn desc(&self) -> &ChunkEncodeDesc {
        &self.0.desc
    }

    fn next_piece(
        &self, 
        session_id: &TempSeq, 
        buf: &mut [u8]
    ) -> BuckyResult<usize> {
        let mut state = self.0.state.write().unwrap();
        match &mut *state {
            StateImpl::Err(err) => Err(BuckyError::new(*err, "encoder failed")), 
            StateImpl::Pending => Ok(0), 
            StateImpl::Ready(ready) => {
                if let Some(index) = ready.index_queue.next() {
                    let buf_len = buf.len();
                    let buf = PieceData::encode_header(
                        buf, 
                        session_id,
                        self.chunk(), 
                        &PieceDesc::Range(index, self.range_size()))?;
                    let header_len = buf_len - buf.len();
                    let piece_len = {
                        if index > self.end_index() {
                            Err(BuckyError::new(BuckyErrorCode::OutOfLimit, "invalid index"))
                        } else if index == self.end_index() {
                            let index = index as usize;
                            let range_size = self.0.range_size as usize;
                            let pre_len = index * range_size;
                            let range_size = self.chunk().len() - pre_len;
                            buf[..range_size].copy_from_slice(&ready.cache[pre_len..self.chunk().len()]);
                            Ok(range_size)
                        } else {
                            let index = index as usize;
                            let range_size = self.0.range_size as usize;
                            buf[..range_size].copy_from_slice(&ready.cache[index * range_size..(index + 1) * range_size]);
                            Ok(range_size)
                        }
                    }?;
                    Ok(header_len + piece_len)
                } else {
                    Ok(0)
                }
            }
        }
    }

    fn reset(
        &self
    ) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        match &mut *state {
            StateImpl::Err(err) => Err(BuckyError::new(*err, "encoder failed")), 
            StateImpl::Pending => Ok(()), 
            StateImpl::Ready(ready) => {
                ready.index_queue = IndexQueue::new(0, self.end_index());
                Ok(())
            }
        }
    }

    fn merge(
        &self, 
        max_index: u32, 
        lost_index: Vec<Range<u32>>
    ) -> BuckyResult<()> {
        let mut state = self.0.state.write().unwrap();
        match &mut *state {
            StateImpl::Err(err) => Err(BuckyError::new(*err, "encoder failed")), 
            StateImpl::Pending => Ok(()), 
            StateImpl::Ready(ready) => {
                debug!("{} will merge max_index:{}, lost_index:{:?}", self, max_index, lost_index);
                ready.index_queue.merge(max_index, lost_index);
                debug!("{} send index queue changed to {:?}", self, ready.index_queue);
                Ok(())
            }
        }
    }
}