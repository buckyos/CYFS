use log::*;
use std::{
    ops::Range, 
    collections::LinkedList, 
    sync::{RwLock}, 
};
use async_std::{
    sync::Arc, 
    task, 
};
use cyfs_base::*;
use crate::{
    types::*, 
};
use super::super::super::{
    channel::protocol::v0::*, 
};
use super::super::{
    storage::ChunkReader
};
use super::{
    encode::*
};

//TODO: Range可以优化内存；不需要保留所有chunk内容在内存;
enum EncoderStateImpl {
    Pending(StateWaiter),
    Ready(Arc<Vec<u8>>), 
    Err(BuckyErrorCode)
}

struct EncoderImpl {
    chunk: ChunkId,
    // range 大小
    range_size: u16, 
    //最后一个index 
    end_index: u32,  
    state: RwLock<EncoderStateImpl>
}

#[derive(Clone)]
pub struct RangeEncoder(Arc<EncoderImpl>);

impl std::fmt::Display for RangeEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RangeEncoder{{chunk:{},range_size:{}}}", self.0.chunk, self.0.range_size)
    }
}

impl RangeEncoder {
    pub fn from_reader(
        reader: Arc<Box<dyn ChunkReader>>, 
        chunk: &ChunkId) -> Self {
        let range_size = PieceData::max_payload();
        let end_index = (chunk.len() + range_size - 1) / range_size - 1;

        let arc_self = Self(Arc::new(EncoderImpl {
            chunk: chunk.clone(), 
            end_index: end_index as u32, 
            range_size: range_size as u16, 
            state: RwLock::new(EncoderStateImpl::Pending(StateWaiter::new())) 
        }));
        trace!("{} begin load from store", arc_self);
        {
            let arc_self = arc_self.clone();
            let chunk = chunk.clone();
            task::spawn(async move {
                let ret = reader.get(&chunk).await;
                let to_wake = {
                    let state =  &mut *arc_self.0.state.write().unwrap();
                    let to_wake = match state {
                        EncoderStateImpl::Pending(state_waiter) => {
                            let to_wake = state_waiter.transfer();
                            to_wake
                        }, 
                        _ => unreachable!()
                    };
                    match ret {
                        Ok(content) => {
                            info!("{} finish read", arc_self);
                            *state = EncoderStateImpl::Ready(content);
                        }, 
                        Err(err) => {
                            error!("{} load chunk failed for {}", arc_self, err);
                            *state = EncoderStateImpl::Err(err.code());
                        }
                    }
                    to_wake
                };
                to_wake.wake();
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

#[async_trait::async_trait]
impl ChunkEncoder for RangeEncoder { 
    fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    fn state(&self) -> ChunkEncoderState {
        match &*self.0.state.read().unwrap() {
            EncoderStateImpl::Ready(_) => ChunkEncoderState::Ready,
            EncoderStateImpl::Pending(_) => ChunkEncoderState::Pending, 
            EncoderStateImpl::Err(err) => ChunkEncoderState::Err(*err)
        }
    }

    async fn wait_ready(&self) -> ChunkEncoderState {
        let (state, waker) = match &mut *self.0.state.write().unwrap() {
            EncoderStateImpl::Ready(_) => (ChunkEncoderState::Ready, None),
            EncoderStateImpl::Pending(state_waiter) =>  (ChunkEncoderState::Pending, Some(state_waiter.new_waiter())), 
            EncoderStateImpl::Err(err) => (ChunkEncoderState::Err(*err), None),
        };
        if let Some(waker) = waker {
            StateWaiter::wait(waker, || self.state()).await
        } else {
            state
        }
    }

    fn piece_of(&self, index: u32, buf: &mut [u8]) -> BuckyResult<usize> {
        match &*self.0.state.read().unwrap() {
            EncoderStateImpl::Err(err) => Err(BuckyError::new(*err, "encoder in error")), 
            EncoderStateImpl::Pending(_) => Err(BuckyError::new(BuckyErrorCode::Pending, "encoder pending on loading")), 
            EncoderStateImpl::Ready(cache) => {
                if index > self.end_index() {
                    Err(BuckyError::new(BuckyErrorCode::OutOfLimit, "invalid index"))
                } else if index == self.end_index() {
                    let index = index as usize;
                    let range_size = self.0.range_size as usize;
                    let pre_len = index * range_size;
                    let range_size = self.chunk().len() - pre_len;
                    buf[..range_size].copy_from_slice(&cache[pre_len..self.chunk().len()]);
                    Ok(range_size)
                } else {
                    let index = index as usize;
                    let range_size = self.0.range_size as usize;
                    buf[..range_size].copy_from_slice(&cache[index * range_size..(index + 1) * range_size]);
                    Ok(range_size)
                }
            }
        }   
    }
}

struct DecodingState {
    pushed: u32, 
    max_index: Option<u32>, 
    lost_index: LinkedList<Range<u32>>, 
    cache: Vec<u8>, 
}

impl DecodingState {
    fn finished_at(&self, index: u32) -> bool {
        if let Some(max_index) = self.max_index.as_ref() {
            *max_index >= index && self.lost_index.len() == 0
        } else {
            false
        }
    }

    fn push_index(&mut self, index: u32) -> bool {
        let pushed = if let Some(max_index) = self.max_index.clone() {
            if index > max_index {
                self.max_index = Some(index);
                if max_index != index - 1 {
                    self.lost_index.push_back((max_index + 1)..index);
                }
                true
            } else if index == max_index {
                false
            } else {
                enum ChangeIndex {
                    None, 
                    Remove(usize), 
                    Insert(usize, Range<u32>)
                }
                let mut change = None;
                for (i, lost) in self.lost_index.iter_mut().enumerate() {
                    if lost.start <= index && lost.end > index {
                        if lost.start == index {
                            if lost.end > index + 1 {
                                lost.start = index + 1;
                                change = Some(ChangeIndex::None);
                            } else {
                                change = Some(ChangeIndex::Remove(i));
                            }
                        } else if lost.end == index + 1 {
                            lost.end = index;
                            change = Some(ChangeIndex::None);
                        } else {
                            let former_end = lost.end;
                            lost.end = index;
                            let next = (index + 1)..former_end;
                            change = Some(ChangeIndex::Insert(i, next));
                        }
                        break;
                    } 
                }
                if let Some(change) = change {
                    match change {
                        ChangeIndex::None => {}, 
                        ChangeIndex::Remove(i) => {
                            let mut last_part = self.lost_index.split_off(i);
                            let _ = last_part.pop_front();
                            self.lost_index.append(&mut last_part);
                        }, 
                        ChangeIndex::Insert(i, range) => {
                            if i + 1 == self.lost_index.len() {
                                self.lost_index.push_back(range);
                            } else {
                                let mut last_part = self.lost_index.split_off(i + 1);
                                last_part.push_front(range);
                                self.lost_index.append(&mut last_part);
                            }
                        }
                    }
                    true
                } else {
                    false
                }
            }
        } else {
            self.max_index = Some(index);
            if index != 0 {
                self.lost_index.push_back(0..index);
            }
            true
        };
        if pushed {
            self.pushed += 1;
        }
        pushed
    }
}

#[test]
fn test_push_index() {
    let mut decoding = DecodingState {
        pushed: 0, 
        max_index: None, 
        lost_index: LinkedList::new(), 
        cache: vec![]
    };

    // let end = 100u32;
    assert!(!decoding.finished_at(0));
    decoding.push_index(0);
    assert!(decoding.finished_at(0));
    decoding.push_index(1);
    assert!(decoding.finished_at(1));
    decoding.push_index(2);
    assert!(decoding.finished_at(2));

    decoding.push_index(4);
    assert!(!decoding.finished_at(4));
    let lost = decoding.lost_index.front().clone().unwrap();
    assert!(lost.start == 3 && lost.end == 4);
    decoding.push_index(3);
    assert!(decoding.finished_at(4));

    decoding.push_index(8);
    let lost = decoding.lost_index.front().clone().unwrap();
    assert!(lost.start == 5 && lost.end == 8);
    decoding.push_index(6);
    assert_eq!(decoding.lost_index.len(), 2);
    let lost = decoding.lost_index.front().clone().unwrap();
    assert!(lost.start == 5 && lost.end == 6);
    let lost = decoding.lost_index.back().clone().unwrap();
    assert!(lost.start == 7 && lost.end == 8);
}   

enum DecoderStateImpl {
    Decoding(DecodingState), 
    Ready(Arc<Vec<u8>>)   
}

//TODO: Range可以优化内存；不需要保留所有chunk内容在内存;
struct DecoderImpl {
    chunk: ChunkId, 
    // range 大小
    range_size: u16, 
    //最后一个index 
    end_index: u32,  
    state: RwLock<DecoderStateImpl>
}

#[derive(Clone)]
pub struct RangeDecoder(Arc<DecoderImpl>);

impl std::fmt::Display for RangeDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RangeDecoder{{chunk:{}}}", self.0.chunk)
    }
}

impl RangeDecoder {
    pub fn new(chunk: &ChunkId) -> Self {
        let range_size = PieceData::max_payload();
        let end_index = (chunk.len() + range_size - 1) / range_size - 1;
        Self(Arc::new(DecoderImpl {
            chunk: chunk.clone(), 
            end_index: end_index as u32, 
            range_size: range_size as u16, 
            state: RwLock::new(DecoderStateImpl::Decoding(DecodingState {
                pushed: 0, 
                max_index: None, 
                lost_index: LinkedList::new(), 
                cache: vec![0u8;chunk.len()]
            }))
        }))
    }

    pub fn range_size(&self) -> u16 {
        self.0.range_size
    }

    pub fn end_index(&self) -> u32 {
        self.0.end_index
    }

    pub fn require_index(&self) -> Option<(Option<u32>, Option<Vec<Range<u32>>>)> {
        let state = &*self.0.state.read().unwrap();
        match state {
            DecoderStateImpl::Decoding(decoding) => {
                Some((
                    decoding.max_index.clone(), 
                    if decoding.lost_index.len() == 0 {
                        None
                    } else {
                        Some(decoding.lost_index.iter().cloned().collect())
                    }))
            }, 
            DecoderStateImpl::Ready(_) => None
        }
    }
}


impl ChunkDecoder for RangeDecoder {
    fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    fn chunk_content(&self) -> Option<Arc<Vec<u8>>> {
        let state = &*self.0.state.read().unwrap();
        match state {
            DecoderStateImpl::Decoding(_) => None, 
            DecoderStateImpl::Ready(chunk) => Some(chunk.clone())
        }
    }

    fn state(&self) -> ChunkDecoderState {
        let state = &*self.0.state.read().unwrap();
        match state {
            DecoderStateImpl::Decoding(decoding) => ChunkDecoderState::Decoding(decoding.pushed), 
            DecoderStateImpl::Ready(_) => ChunkDecoderState::Ready
        } 
    }

    fn push_piece_data(&self, piece: &PieceData) -> (ChunkDecoderState, ChunkDecoderState) {
        trace!("{} push piece desc {:?}", self, piece.desc);
        let index = piece.desc.range_index(self.range_size()).unwrap();
        let state = &mut *self.0.state.write().unwrap();
        match state {
            DecoderStateImpl::Decoding(decoding) => {
                let pushed = decoding.pushed;
                if index > self.end_index() {
                    (ChunkDecoderState::Decoding(pushed), ChunkDecoderState::Decoding(pushed))
                } else {
                    if decoding.push_index(index) {
                        let index = index as usize;
                        let range_size = self.range_size() as usize;
                        let buf_range = if index == self.end_index() as usize {
                            index * range_size..self.chunk().len()
                        } else {
                            index * range_size..(index + 1) * range_size
                        };
                        decoding.cache.as_mut_slice()[buf_range].copy_from_slice(piece.data.as_slice());
                        if decoding.finished_at(self.end_index()) {
                            let mut content = vec![];
                            std::mem::swap(&mut content, &mut decoding.cache);
                            *state = DecoderStateImpl::Ready(Arc::new(content));
                            (ChunkDecoderState::Decoding(pushed), ChunkDecoderState::Ready)
                        } else {
                            (ChunkDecoderState::Decoding(pushed), ChunkDecoderState::Decoding(decoding.pushed))
                        }
                    } else {
                        (ChunkDecoderState::Decoding(pushed), ChunkDecoderState::Decoding(pushed))
                    }
                }
            }, 
            DecoderStateImpl::Ready(_) => {
                trace!("{} ingnore piece seq {} for decoder is ready", self, index);
                (ChunkDecoderState::Ready, ChunkDecoderState::Ready)
            }
        }
    }
}
