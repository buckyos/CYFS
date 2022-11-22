use std::{
    collections::{LinkedList}, 
    sync::{Arc, RwLock}, 
    io::SeekFrom, 
};
use async_std::{
    pin::Pin, 
    task::{Context, Poll},
    task
};

use cyfs_base::*;
use crate::{
    types::*
};
use super::super::{
    types::*, 
    chunk::*,
    channel::protocol::v0::*
};


pub trait DownloadContext: Send + Sync {
    fn clone_as_context(&self) -> Box<dyn DownloadContext>;
    fn referer(&self) -> &str;
    fn source_exists(&self, target: &DeviceId, encode_desc: &ChunkEncodeDesc) -> bool;
    fn sources_of(&self, filter: Box<dyn Fn(&DownloadSource) -> bool>, limit: usize) -> LinkedList<DownloadSource>;
}

#[derive(Clone)]
pub struct DownloadSource {
    pub target: DeviceDesc, 
    pub encode_desc: ChunkEncodeDesc, 
}


pub struct DownloadSourceWithReferer<T: Send + Sync> {
    pub target: T, 
    pub encode_desc: ChunkEncodeDesc, 
    pub referer: String, 
    context_id: IncreaseId 
}

impl Into<DownloadSourceWithReferer<DeviceId>> for DownloadSourceWithReferer<DeviceDesc> {
    fn into(self) -> DownloadSourceWithReferer<DeviceId> {
        DownloadSourceWithReferer {
            target: self.target.device_id(), 
            encode_desc: self.encode_desc, 
            referer: self.referer, 
            context_id: self.context_id 
        }
    }
}

struct MultiContextImpl {
    gen_id: IncreaseIdGenerator, 
    contexts: LinkedList<(IncreaseId, Box<dyn DownloadContext>)>
}

#[derive(Clone)]
pub struct MultiDownloadContext(Arc<RwLock<MultiContextImpl>>);

impl MultiDownloadContext {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(MultiContextImpl {
            gen_id: IncreaseIdGenerator::new(), 
            contexts: (LinkedList::new())
        })))
    }

    pub fn add_context(&self, context: &dyn DownloadContext) -> IncreaseId {
        let mut state = self.0.write().unwrap();
        let id = state.gen_id.generate();
        state.contexts.push_back((id, context.clone_as_context()));
        id
    }


    pub fn remove_context(&self, remove_id: &IncreaseId) {
        let mut state = self.0.write().unwrap();
        
        if let Some((index, _)) = state.contexts.iter().enumerate().find(|(_, (id, _))| id.eq(remove_id)) {
            let mut back_parts = state.contexts.split_off(index);
            let _ = back_parts.pop_front();
            state.contexts.append(&mut back_parts);
            // contexts.remove(index);
        }
    }

    pub fn sources_of(&self, filter: impl Fn(&DownloadSource) -> bool + Copy + 'static, limit: usize) -> LinkedList<DownloadSourceWithReferer<DeviceDesc>> {
        let mut result = LinkedList::new();
        let mut limit = limit;
        let state = self.0.read().unwrap();
        for (id, context) in state.contexts.iter() {
            let part = context.sources_of(Box::new(filter), limit);
            limit -= part.len();
            for source in part {
                result.push_back(DownloadSourceWithReferer {
                    target: source.target, 
                    encode_desc: source.encode_desc, 
                    referer: context.referer().to_owned(), 
                    context_id: *id  
                });
            }
            if limit == 0 {
                break;
            }
        }   
        result
    }

    pub fn source_exists(&self, source: &DownloadSourceWithReferer<DeviceId>) -> bool {
        let state = self.0.read().unwrap();
        state.contexts.iter().find(|(id, context)|{
            if source.context_id.eq(id) {
                context.source_exists(&source.target, &source.encode_desc)
            } else {
                false
            }
        }).is_some()
    }
}


#[derive(Clone, Copy)]
pub enum DownloadTaskPriority {
    Backgroud = 1, 
    Normal = 2, 
    Realtime = 4,
}

impl Default for DownloadTaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}


// 对scheduler的接口
#[derive(Debug)]
pub enum DownloadTaskState {
    Downloading(u32/*速度*/, f32/*进度*/),
    Paused,
    Error(BuckyError/*被cancel的原因*/), 
    Finished
}

#[derive(Clone, Debug)]
pub enum DownloadTaskControlState {
    Normal, 
    Paused, 
    Canceled, 
}

#[async_trait::async_trait]
pub trait DownloadTask: Send + Sync {
    fn clone_as_task(&self) -> Box<dyn DownloadTask>;
    fn state(&self) -> DownloadTaskState;
    fn control_state(&self) -> DownloadTaskControlState;
    async fn wait_user_canceled(&self) -> BuckyError;

    fn resume(&self) -> BuckyResult<DownloadTaskControlState> {
        Ok(DownloadTaskControlState::Normal)
    }
    fn cancel(&self) -> BuckyResult<DownloadTaskControlState> {
        Ok(DownloadTaskControlState::Normal)
    }
    fn pause(&self) -> BuckyResult<DownloadTaskControlState> {
        Ok(DownloadTaskControlState::Normal)
    }

    fn priority_score(&self) -> u8 {
        DownloadTaskPriority::Normal as u8
    }
    fn add_task(&self, _path: Option<String>, _sub: Box<dyn DownloadTask>) -> BuckyResult<()> {
        Err(BuckyError::new(BuckyErrorCode::NotSupport, "no implement"))
    }
    fn sub_task(&self, _path: &str) -> Option<Box<dyn DownloadTask>> {
        None
    }
    fn close(&self) -> BuckyResult<()> {
        Ok(())
    }

    fn calc_speed(&self, when: Timestamp) -> u32;
    fn cur_speed(&self) -> u32;
    fn history_speed(&self) -> u32;

    fn drain_score(&self) -> i64 {
        0
    }
    fn on_drain(&self, expect_speed: u32) -> u32;
}


pub struct DownloadTaskReader {
    cache: ChunkCache, 
    offset: usize,
    task: Box<dyn DownloadTask>
}

impl std::fmt::Display for DownloadTaskReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DownloadTaskReader{{chunk:{}}}", self.cache.chunk())
    }
}

impl DownloadTaskReader {
    pub fn new(cache: ChunkCache, task: Box<dyn DownloadTask>) -> Self {
        Self {
            cache, 
            offset: 0, 
            task
        }
    }

    pub fn task(&self) -> &dyn DownloadTask {
        self.task.as_ref()
    }
}

impl std::io::Seek for DownloadTaskReader {
    fn seek(
        self: &mut Self,
        pos: SeekFrom,
    ) -> std::io::Result<u64> {
        let len = self.cache.chunk().len();
        let new_offset = match pos {
            SeekFrom::Start(offset) => len.min(offset as usize), 
            SeekFrom::Current(offset) => {
                let offset = (self.offset as i64) + offset;
                let offset = offset.max(0);
                len.min(offset as usize)
            },
            SeekFrom::End(offset) => {
                let offset = (len as i64) + offset;
                let offset = offset.max(0);
                len.min(offset as usize)
            }
        };
        self.offset = new_offset;

        Ok(new_offset as u64)   
    }
}

impl async_std::io::Read for DownloadTaskReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let pined = self.get_mut();
        trace!("{} poll_read: {} offset: {}", pined, buffer.len(), pined.offset);
        if let DownloadTaskState::Error(err) = pined.task.state() {
            trace!("{} poll_read: {} offset: {} error: {}", pined, buffer.len(), pined.offset, err);
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, BuckyError::new(err, ""))));
        } 
        if let Some(range) = pined.cache.exists(pined.offset..pined.offset + buffer.len()) {
            trace!("{} poll_read: {} offset: {} exists {:?}", pined, buffer.len(), pined.offset, range);
            let (desc, mut offset) = PieceDesc::from_stream_offset(PieceData::max_payload(), range.start as u32);
            let (mut index, len) = desc.unwrap_as_stream();
            let mut read = 0;
            let result = loop {
                match pined.cache.stream().sync_try_read(
                    &PieceDesc::Range(index, len), 
                    offset as usize, 
                    &mut buffer[read..]) {
                    Ok(this_read) => {
                        read += this_read;
                        if this_read == 0 
                            || read >= buffer.len() {
                            pined.offset += read;
                            break Ok(read);
                        }
                        index += 1;
                        offset = 0;
                    },
                    Err(err) => {
                        break Err(std::io::Error::new(std::io::ErrorKind::Other, err))
                    }
                }
            };
            Poll::Ready(result)
        } else {
            let waker = cx.waker().clone();
            let cache = pined.cache.clone();
            let task = pined.task.clone_as_task();
            let range = pined.offset..pined.offset + buffer.len();
            task::spawn(async move {
                let _ = cache.wait_exists(range, || task.wait_user_canceled()).await;
                waker.wake();
            });
            Poll::Pending
        }
    }
}
