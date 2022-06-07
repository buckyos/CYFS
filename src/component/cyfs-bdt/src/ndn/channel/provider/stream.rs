use std::{
    ops::Range, 
    collections::LinkedList, 
    time::Duration, 
    sync::{atomic::{AtomicU64, AtomicU16, Ordering}}
};
use async_std::{
    sync::Arc
};
use cyfs_base::*;
use crate::{
    types::*
};
use super::super::super::{
    chunk::*, 
};
use super::super::{
    protocol::*, 
    channel::Channel
};
use super::provider::*; 
use cyfs_debug::Mutex;

struct DownloadImpl {
    session_id: TempSeq, 
    channel: Channel, 
    decoder: RangeDecoder, 
    last_pushed: AtomicU64, 
    loss_count: AtomicU16
}

#[derive(Clone)]
pub struct StreamDownload(Arc<DownloadImpl>);

impl std::fmt::Display for StreamDownload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamDownload{{chunk:{},remote:{}}}", self.decoder().chunk(), self.channel().remote())
    }
}


impl StreamDownload {
    pub fn new(chunk: &ChunkId, session_id: TempSeq, channel: Channel) -> Self {
        Self(Arc::new(DownloadImpl {
            session_id, 
            channel, 
            decoder: RangeDecoder::new(chunk), 
            last_pushed: AtomicU64::new(0), 
            loss_count: AtomicU16::new(0)
        }))
    }

    pub fn channel(&self) -> &Channel {
        &self.0.channel
    }
}

impl DownloadSessionProvider for StreamDownload {
    fn decoder(&self) -> &dyn ChunkDecoder {
        &self.0.decoder
    }
    
    fn clone_as_provider(&self) -> Box<dyn DownloadSessionProvider> {
        Box::new(self.clone())
    }

    fn on_time_escape(&self, now: Timestamp) -> BuckyResult<()> {
        let last_pushed = self.0.last_pushed.load(Ordering::SeqCst);
        if now > last_pushed 
            && Duration::from_micros(now - last_pushed) > self.channel().config().resend_interval {
            if let Some((max_index, lost_index)) = self.0.decoder.require_index() {
                self.0.last_pushed.store(now, Ordering::SeqCst);
                if self.channel().config().resend_interval * self.0.loss_count.fetch_add(1, Ordering::SeqCst).into()
                    > self.channel().config().resend_timeout {
                    error!("{} break", self);
                    Err(BuckyError::new(BuckyErrorCode::Timeout, "downloader break for no data recved"))
                } else {
                    debug!("{} dectect loss piece max_index:{:?} lost_index:{:?}", self, max_index, lost_index);
                    let ctrl = PieceControl {
                        sequence: self.channel().gen_command_seq(), 
                        session_id: self.0.session_id.clone(), 
                        chunk: self.0.decoder.chunk().clone(), 
                        command: PieceControlCommand::Continue, 
                        max_index, 
                        lost_index
                    };
                    self.0.channel.send_piece_control(ctrl);
                    Ok(())
                }
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn push_piece_data(&self, piece: &PieceData) -> BuckyResult<bool/*finished*/> {
        let (pre_state, next_state) = self.0.decoder.push_piece_data(piece);
        if pre_state != next_state {
            self.0.last_pushed.store(bucky_time_now(), Ordering::SeqCst);
            self.0.loss_count.store(0, Ordering::SeqCst);
        }

        Ok(next_state == ChunkDecoderState::Ready)
    }
}


struct IndexQueue {
    end: u32, 
    queue: LinkedList<Range<u32>>
}

impl IndexQueue {
    fn new(start: u32, end: u32) -> Self {
        let mut queue = LinkedList::new();
        queue.push_back(start..end + 1);
        Self {
            end, 
            queue 
        }
    }

    fn merge(&mut self, max_index: u32, lost_index: &Option<Vec<Range<u32>>>) {
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
        if let Some(lost_index) = lost_index.as_ref() {
            for lost in lost_index {
                skip = merge_one(lost.clone(), skip);
            }
        }

        if max_index < end_index {
            merge_one(max_index + 1..end_index + 1, skip);
        }
    } 

    fn next(&mut self) -> Option<u32> {
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

    queue.merge(5, &None);
    assert_eq!(queue.next(), Some(6));

    queue.merge(4, &None);
    assert_eq!(queue.next(), Some(5));
    assert_eq!(queue.next(), Some(6));
    assert_eq!(queue.next(), Some(7));
    assert_eq!(queue.next(), Some(8));
    assert_eq!(queue.next(), Some(9));
    assert_eq!(queue.next(), None);
}

struct UploadImpl {
    session_id: TempSeq, 
    encoder: RangeEncoder, 
    index_queue: Mutex<IndexQueue>
}

#[derive(Clone)]
pub struct StreamUpload(Arc<UploadImpl>);

impl std::fmt::Display for StreamUpload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamUpload{{chunk:{}}}", self.0.encoder.chunk())
    }
}

impl StreamUpload {
    pub fn new(session_id: TempSeq, encoder: RangeEncoder) -> Self {
        let end_index = encoder.end_index();
        Self(Arc::new(UploadImpl {
            session_id, 
            encoder, 
            index_queue: Mutex::new(IndexQueue::new(0, end_index))
        }))
    }
}


impl UploadSessionProvider for StreamUpload {
    fn state(&self) -> ChunkEncoderState {
        self.0.encoder.state()
    }

    fn clone_as_provider(&self) -> Box<dyn UploadSessionProvider> {
        Box::new(self.clone())
    }

    fn next_piece(
        &self, 
        buf: &mut [u8]) -> BuckyResult<usize> {
        //TODO: 这里可以配合 encoder的实现，使用部分缓存
        match self.0.encoder.state() {
            ChunkEncoderState::Err(err) => Err(BuckyError::new(err, "encoder failed")), 
            ChunkEncoderState::Pending => Ok(0), 
            ChunkEncoderState::Ready => {
                let next_index = {   
                    let mut index_queue = self.0.index_queue.lock().unwrap();
                    let next_index = index_queue.next();
                    trace!("{} send index {:?} remain {:?}", self, next_index, index_queue.queue);
                    next_index
                };
                if let Some(index) = next_index {
                    let buf_len = buf.len();
                    let buf = PieceData::encode_header(
                        buf, 
                        &self.0.session_id,
                        self.0.encoder.chunk(), 
                        &PieceDesc::Range(index, self.0.encoder.range_size()))?;
                    let header_len = buf_len - buf.len();
                    let piece_len = self.0.encoder.piece_of(index, buf).unwrap();
                    Ok(header_len + piece_len)
                } else {
                    Ok(0)
                }
            }
        }
    }

    fn on_interest(&self, _interest: &Interest) -> BuckyResult<()> {
        debug!("{} will reset index", self);
        let mut index_queue = self.0.index_queue.lock().unwrap();
        *index_queue = IndexQueue::new(0, self.0.encoder.end_index());
        Ok(())
    }

    fn on_piece_control(&self, control: &PieceControl) -> BuckyResult<()> {
        match &control.command {
            PieceControlCommand::Continue => {
                if let Some(max_index) = control.max_index {
                    debug!("{} will merge max_index:{}, lost_index:{:?}", self, max_index, control.lost_index);
                    let mut index_queue = self.0.index_queue.lock().unwrap();
                    index_queue.merge(max_index, &control.lost_index);
                    debug!("{} send index queue changed to {:?}", self, index_queue.queue);
                }
            }, 
            _ => {}
        }
        Ok(())
    }
}