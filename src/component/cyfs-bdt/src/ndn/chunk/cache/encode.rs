use std::{
    collections::LinkedList, 
    ops::Range, 
    sync::Arc
};
use cyfs_base::*;
use crate::{
    types::*
};
use super::super::super::{
    channel::{protocol::v0::*}, 
    types::*
};

#[derive(Clone, Eq, PartialEq)]
pub enum ChunkDecoderState2 {
    Decoding(u32), 
    Ready, 
}

pub trait ChunkDecoder2: Send + Sync {
    fn clone_as_decoder(&self) -> Box<dyn ChunkDecoder2>;
    fn chunk(&self) -> &ChunkId;
    fn desc(&self) -> &ChunkEncodeDesc;
    fn require_index(&self) -> Option<(Option<u32>, Option<Vec<Range<u32>>>)>;
    fn push_piece_data(&self, piece: &PieceData) -> (ChunkDecoderState2, ChunkDecoderState2);
    fn chunk_content(&self) -> Option<Arc<Vec<u8>>>;
}

pub trait ChunkEncoder2: Send + Sync {
    fn clone_as_encoder(&self) -> Box<dyn ChunkEncoder2>;
    fn chunk(&self) -> &ChunkId;
    fn desc(&self) -> &ChunkEncodeDesc;
    fn next_piece(
        &self, 
        session_id: &TempSeq, 
        buf: &mut [u8]
    ) -> BuckyResult<usize>;
    fn reset(&self) -> BuckyResult<()>;
    fn merge(
        &self, 
        max_index: u32, 
        lost_index: Vec<Range<u32>>
    ) -> BuckyResult<()>;
}



#[derive(Debug)]
pub(crate) struct IndexQueue {
    end: u32, 
    queue: LinkedList<Range<u32>>
}


impl IndexQueue {
    pub fn new(start: u32, end: u32) -> Self {
        let mut queue = LinkedList::new();
        queue.push_back(start..end + 1);
        Self {
            end, 
            queue 
        }
    }

    pub fn merge(&mut self, max_index: u32, lost_index: Vec<Range<u32>>) {
        let end_index = self.end;

        enum ChangeQueue {
            None, 
            Insert(usize), 
            CheckMerge(usize), 
            PushBack
        }

        let mut merge_one = |lost: Range<u32>, skip| {
            if self.queue.len() > 0 {
                let mut change = ChangeQueue::PushBack;
                let mut skip = skip;
                for (i, next) in self.queue.iter_mut().enumerate().skip(skip) {
                    if lost.start >= next.start 
                    && lost.end <= next.end {
                        // 最常见的情况，包含在其中
                        change = ChangeQueue::None;
                        break;
                    } else if lost.end < next.start {
                        // 朝前附加
                        ChangeQueue::Insert(i);
                        break;
                    } else if lost.end == next.start {
                        // 和当前合并
                        next.start = lost.start;
                        change = ChangeQueue::None;
                        break;
                    } else if lost.start <= next.end {
                        // 扩展当前，检查后面的是否合并
                        next.start = std::cmp::min(lost.start, next.start);
                        next.end = lost.end;
                        change = ChangeQueue::CheckMerge(i);
                        break;
                    } else {
                        skip += 1;
                        continue;
                    }
                }

                match change {
                    ChangeQueue::None => {
                        // skip 不变
                    },  
                    ChangeQueue::Insert(i) => {
                        let mut last_part = self.queue.split_off(i);
                        last_part.push_front(lost);
                        self.queue.append(&mut last_part);
                        skip += 1;
                    },
                    ChangeQueue::CheckMerge(i) => {
                        let mut merged_len = 0;
                        let mut iter = self.queue.iter().skip(i);
                        let base = iter.next().unwrap().clone();
                        for next in iter {
                            if next.start > base.end {
                                break;
                            } 
                            merged_len += 1;
                        }
                        if merged_len > 0 {
                            let mut last_part = self.queue.split_off(i + 1);
                            let mut append_back = last_part.split_off(merged_len);
                            let base_ref = self.queue.back_mut().unwrap();
                            let merge_end = last_part.back().unwrap().end;
                            if base_ref.end < merge_end {
                                base_ref.end = merge_end;
                            }
                            self.queue.append(&mut append_back);
                        }
                    }, 
                    ChangeQueue::PushBack => {
                        self.queue.push_back(lost);
                        skip += 1;
                    }
                }
                skip
            } else {
                self.queue.push_back(lost);
                1
            }
        };
        

        let mut skip = 0;
        for lost in lost_index {
            skip = merge_one(lost.clone(), skip);
        }

        if max_index < end_index {
            merge_one(max_index + 1..end_index + 1, skip);
        }
    } 

    pub fn next(&mut self) -> Option<u32> {
        if let Some(range) = self.queue.front_mut() {
            let index = if range.end - range.start == 1 {
                self.queue.pop_front().unwrap().start
            } else {
                let index = range.start;
                range.start += 1;
                index
            };
            Some(index)
        } else {
            None
        }
    }
}


#[test]
fn test_index_queue() {
    let mut queue = IndexQueue::new(0, 9);
    assert_eq!(queue.next(), Some(0));
    assert_eq!(queue.next(), Some(1));
    assert_eq!(queue.next(), Some(2));
    assert_eq!(queue.next(), Some(3));
    assert_eq!(queue.next(), Some(4));
    assert_eq!(queue.next(), Some(5));

    queue.merge(5, vec![]);
    assert_eq!(queue.next(), Some(6));

    queue.merge(4, vec![]);
    assert_eq!(queue.next(), Some(5));
    assert_eq!(queue.next(), Some(6));
    assert_eq!(queue.next(), Some(7));
    assert_eq!(queue.next(), Some(8));
    assert_eq!(queue.next(), Some(9));
    assert_eq!(queue.next(), None);
}


