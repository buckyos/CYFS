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
pub enum ChunkDecoderState {
    Decoding(u32), 
    Ready, 
}

pub trait ChunkDecoder: Send + Sync {
    fn clone_as_decoder(&self) -> Box<dyn ChunkDecoder>;
    fn chunk(&self) -> &ChunkId;
    fn desc(&self) -> &ChunkEncodeDesc;
    fn require_index(&self) -> Option<(Option<u32>, Option<Vec<Range<u32>>>)>;
    fn push_piece_data(&self, piece: &PieceData) -> BuckyResult<PushIndexResult>;
}

pub trait ChunkEncoder: Send + Sync {
    fn clone_as_encoder(&self) -> Box<dyn ChunkEncoder>;
    fn chunk(&self) -> &ChunkId;
    fn desc(&self) -> &ChunkEncodeDesc;
    fn next_piece(
        &self, 
        session_id: &TempSeq, 
        buf: &mut [u8]
    ) -> BuckyResult<usize>;
    fn reset(&self);
    fn merge(
        &self, 
        max_index: u32, 
        lost_index: Vec<Range<u32>>
    );
}


#[derive(Clone, Copy)]
pub struct PushIndexResult {
    pub valid: bool, 
    pub exists: bool, 
    pub finished: bool
}

impl PushIndexResult {
    pub fn pushed(&self) -> bool {
        !self.finished && !self.exists
    }
}

pub struct IncomeIndexQueue {
    end: u32, 
    queue: LinkedList<Range<u32>>
}

impl IncomeIndexQueue {
    pub fn new(end: u32) -> Self {
        Self {
            end, 
            queue: LinkedList::new()
        }
    }

    pub fn require(&self, start: u32, end: u32, step: i32) -> Option<(Option<u32>, Option<Vec<Range<u32>>>)> {
        if self.finished() {
            return None;
        }

        let mut require = LinkedList::new(); 
        let mut cur_range: Option<Range<u32>> = None;

        for exists in self.queue.iter() {
            if exists.end <= start {
                continue;
            }
            if start >= exists.start && end <= exists.end {
                break;
            } 
            if exists.start >= end {
                assert!(cur_range.is_some());
                cur_range.as_mut().unwrap().end = end;
                require.push_back(cur_range.clone().unwrap());
                cur_range = None;
                break;
            }
            if exists.start < start && exists.end > start {
                assert!(cur_range.is_none());
                cur_range = Some(exists.end..0);
                continue;
            }
            if exists.start > start && exists.end < end {
                assert!(cur_range.is_some());
                cur_range.as_mut().unwrap().end = exists.start;
                require.push_back(cur_range.clone().unwrap());
                cur_range = Some(exists.end..0);
                continue;
            }   
        }
        
        if require.len() > 0 {
            if step > 0 {
                Some((Some(self.queue.back().unwrap().end - 1), Some(require.into_iter().collect())))
            } else {
                Some((Some(self.queue.front().unwrap().start), Some(require.into_iter().collect())))
            }
        } else {
            None
        }
    }

    pub fn finished(&self) -> bool {
        if self.queue.len() != 1 {
            return false;
        }
        let index = self.queue.front().unwrap();
        index.start == 0 && index.end == self.end
    }

    pub fn try_push(&self, index: Range<u32>) -> PushIndexResult {
        if index.start >= self.end {
            return PushIndexResult {
                valid: false, 
                exists: false,
                finished: self.finished()
            };
        }

        let mut exists = false;
        
        for range in self.queue.iter() {
            if index.start >= range.start && index.end < range.end {
                exists = true;
                break;
            } 
        }
        
        PushIndexResult {
            valid: true, 
            exists,
            finished: self.finished()
        }
    }

    pub fn push(&mut self, index: Range<u32>) -> PushIndexResult {
        if index.start >= self.end {
            return PushIndexResult {
                valid: false, 
                exists: false,
                finished: self.finished()
            };
        }
        
        enum ChangeQueue {
            None, 
            Insert(usize), 
            CheckMerge(usize), 
            PushBack
        }

        let mut exists = false;
        if self.queue.len() > 0 {
            let mut change = ChangeQueue::PushBack;
            for (i, next) in self.queue.iter_mut().enumerate() {
                if index.start >= next.start 
                    && index.end <= next.end {
                    // 最常见的情况，包含在其中
                    change = ChangeQueue::None;
                    exists = true;
                    break;
                } else if index.end < next.start {
                    // 朝前附加
                    ChangeQueue::Insert(i);
                    break;
                } else if index.end == next.start {
                    // 和当前合并
                    next.start = index.start;
                    change = ChangeQueue::None;
                    break;
                } else if index.start <= next.end {
                    // 扩展当前，检查后面的是否合并
                    next.start = std::cmp::min(index.start, next.start);
                    next.end = index.end;
                    change = ChangeQueue::CheckMerge(i);
                    break;
                } else {
                    continue;
                }
            }

            
            match change {
                ChangeQueue::None => {
                    // skip 不变
                },  
                ChangeQueue::Insert(i) => {
                    let mut last_part = self.queue.split_off(i);
                    last_part.push_front(index);
                    self.queue.append(&mut last_part);
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
                    self.queue.push_back(index);
                }
            };
        } else {
            self.queue.push_back(index);
        }

        PushIndexResult {
            valid: true, 
            exists, 
            finished: self.finished()
        }
        
    }

    pub fn exists(&self, index: u32) -> bool {
        for exists in self.queue.iter() {
            if index >= exists.start && index < exists.end {
                return true;
            } 
        }
        false
    }
}



#[test]
fn test_income_index_queue() {
    let mut indices = IncomeIndexQueue {
        end: 10u32, 
        queue: LinkedList::new()
    };

    indices.push(0..1);
}   




#[derive(Debug)]
pub struct OutcomeIndexQueue {
    step: i32, 
    start: u32, 
    end: u32, 
    queue: LinkedList<Range<u32>>
}


impl OutcomeIndexQueue {
    pub fn new(start: u32, end: u32, step: i32) -> Self {
        let mut queue = LinkedList::new();
        queue.push_back(start..end + 1);
        Self {
            step, 
            start, 
            end, 
            queue 
        }
    }

    pub fn reset(&mut self) {
        let mut queue = LinkedList::new();
        queue.push_back(self.start..self.end + 1);
        self.queue = queue;
    }

    pub fn merge(&mut self, max_index: u32, lost_index: Vec<Range<u32>>) {
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

        if self.step > 0 {
            if max_index < self.end {
                merge_one(max_index + 1..self.end + 1, skip);
            }
        } else {
            if max_index > self.start {
                merge_one(self.start..max_index - 1, skip);
            }
        }
        
    } 

    pub fn next(&self) -> Option<u32> {
        if self.step > 0 {
            self.queue.front().map(|r| r.start)
        } else {
            self.queue.back().map(|r| r.end - 1)
        }
    }

    pub fn pop_next(&mut self) -> Option<u32> {
        if self.queue.len() > 0 {
            if self.step > 0 {
                let range = self.queue.front_mut().unwrap();
                let index = if range.end - range.start == 1 {
                    self.queue.pop_front().unwrap().start
                } else {
                    let index = range.start;
                    range.start += 1;
                    index
                };
                Some(index)
            } else {
                let range = self.queue.back_mut().unwrap();
                let index = if range.end - range.start == 1 {
                    self.queue.pop_back().unwrap().end - 1
                } else {
                    let index = range.end - 1;
                    range.end -= 1;
                    index
                };
                Some(index)
            }
        } else {
            None
        }
    }
}


#[test]
fn test_outcome_index_queue() {
    let mut queue = OutcomeIndexQueue::new(0, 9, 1);
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



    let mut queue = OutcomeIndexQueue::new(0, 9, -1);
    assert_eq!(queue.next(), Some(9));
    assert_eq!(queue.next(), Some(8));
    assert_eq!(queue.next(), Some(7));
    assert_eq!(queue.next(), Some(6));
    assert_eq!(queue.next(), Some(5));
    assert_eq!(queue.next(), Some(4));

    queue.merge(5, vec![]);
    assert_eq!(queue.next(), Some(4));

    queue.merge(5, vec![]);
    assert_eq!(queue.next(), Some(4));
    assert_eq!(queue.next(), Some(3));
    assert_eq!(queue.next(), Some(2));
    assert_eq!(queue.next(), Some(1));
    assert_eq!(queue.next(), Some(0));
    assert_eq!(queue.next(), None);
}




