use std::{
    sync::{Arc, RwLock}, 
    ops::Range, 
    collections::{LinkedList}
};
use cyfs_base::*;
use super::super::super::super::{
    types::*, 
    channel::{protocol::v0::*}
};
use super::super::{
    encode::*
};


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


struct DecoderImpl {
    chunk: ChunkId, 
    desc: ChunkEncodeDesc, 
    // range 大小
    range_size: u16, 
    //最后一个index 
    end_index: u32,  
    state: RwLock<DecoderStateImpl>
}

#[derive(Clone)]
pub struct StreamDecoder(Arc<DecoderImpl>);


impl std::fmt::Display for StreamDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamDecoder{{chunk:{}}}", self.chunk())
    }
}

impl StreamDecoder {
    pub fn new(chunk: &ChunkId, desc: &ChunkEncodeDesc) -> Self {
        let range_size = PieceData::max_payload();
        let end_index = (chunk.len() + range_size - 1) / range_size - 1;
        Self(Arc::new(DecoderImpl {
            chunk: chunk.clone(), 
            desc: desc.clone(), 
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

}

impl ChunkDecoder2 for StreamDecoder {
    fn clone_as_decoder(&self) -> Box<dyn ChunkDecoder2> {
        Box::new(self.clone())
    }

    fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    fn desc(&self) -> &ChunkEncodeDesc {
        &self.0.desc
    }

    fn chunk_content(&self) -> Option<Arc<Vec<u8>>> {
        let state = &*self.0.state.read().unwrap();
        match state {
            DecoderStateImpl::Decoding(_) => None,
            DecoderStateImpl::Ready(cache) => Some(cache.clone()) 
        }
    }

    fn require_index(&self) -> Option<(Option<u32>, Option<Vec<Range<u32>>>)> {
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

    fn push_piece_data(&self, piece: &PieceData) -> (ChunkDecoderState2, ChunkDecoderState2) {
        trace!("{} push piece desc {:?}", self, piece.desc);
        let index = piece.desc.range_index(self.range_size()).unwrap();
        let state = &mut *self.0.state.write().unwrap();
        match state {
            DecoderStateImpl::Decoding(decoding) => {
                let pushed = decoding.pushed;
                if index > self.end_index() {
                    (ChunkDecoderState2::Decoding(pushed), ChunkDecoderState2::Decoding(pushed))
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
                            (ChunkDecoderState2::Decoding(pushed), ChunkDecoderState2::Ready)
                        } else {
                            (ChunkDecoderState2::Decoding(pushed), ChunkDecoderState2::Decoding(decoding.pushed))
                        }
                    } else {
                        (ChunkDecoderState2::Decoding(pushed), ChunkDecoderState2::Decoding(pushed))
                    }
                }
            }, 
            DecoderStateImpl::Ready(_) => {
                trace!("{} ingnore piece seq {} for decoder is ready", self, index);
                (ChunkDecoderState2::Ready, ChunkDecoderState2::Ready)
            }
        }
    }

}

