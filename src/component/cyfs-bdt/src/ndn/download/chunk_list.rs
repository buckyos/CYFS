use std::{
    sync::{RwLock},
    io::SeekFrom, 
    ops::Range
};
use async_std::{
    sync::Arc, 
    pin::Pin, 
    task::{Context, Poll}, 
};

use cyfs_base::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}, 
};
use super::super::{
    chunk::*, 
    types::*
};
use super::{
    common::*, 
};

struct DownloadingChunk {
    downloader: ChunkDownloader, 
}

struct DownloadingState { 
    cur_chunk: DownloadingChunk, 
    history_speed: HistorySpeed,
    drain_score: i64
}

enum ControlStateImpl {
    Normal(StateWaiter), 
    Canceled,
}

enum TaskStateImpl {
    Pending, 
    Downloading(DownloadingState), 
    Error(BuckyError), 
    Finished
}

struct StateImpl {
    abs_path: Option<String>, 
    control_state: ControlStateImpl, 
    task_state: TaskStateImpl,
}

struct TaskImpl {
    stack: WeakStack, 
    name: String, 
    chunk_list: ChunkListDesc, 
    context: Box<dyn DownloadContext>, 
    state: RwLock<StateImpl>,  
}

#[derive(Clone)]
pub struct ChunkListTask(Arc<TaskImpl>);

impl std::fmt::Display for ChunkListTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkListTask::{{name:{}}}", self.name())
    }
}

impl ChunkListTask {
    pub fn new(
        stack: WeakStack,  
        name: String,
        chunk_list: ChunkListDesc, 
        context: Box<dyn DownloadContext>, 
    ) -> Self {
        Self(Arc::new(TaskImpl {
            stack, 
            name, 
            context, 
            state: RwLock::new(StateImpl {
                abs_path: None, 
                task_state: TaskStateImpl::Pending,
                control_state: ControlStateImpl::Normal(StateWaiter::new()),
            }), 
            chunk_list, 
        }))
    } 

    pub fn name(&self) -> &str {
        self.0.name.as_str()
    }

    pub fn chunk_list(&self) -> &ChunkListDesc {
        &self.0.chunk_list
    }

    fn create_cache(&self, index: usize) -> BuckyResult<ChunkCache> {
        let stack = Stack::from(&self.0.stack);
        let chunk = &self.chunk_list().chunks()[index];

        let downloader = stack.ndn().chunk_manager().create_downloader(chunk, self.clone_as_leaf_task());
        let cache = downloader.cache().clone();
        let _ = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.task_state {
                TaskStateImpl::Pending => {
                    state.task_state = TaskStateImpl::Downloading(DownloadingState { 
                        cur_chunk: DownloadingChunk {
                            downloader, 
                        }, 
                        history_speed: HistorySpeed::new(0, stack.config().ndn.channel.history_speed.clone()), 
                        drain_score: 0 
                    });
                    Ok(())
                }, 
                TaskStateImpl::Downloading(downloading) => {
                    downloading.cur_chunk = DownloadingChunk {
                        downloader, 
                    };
                    Ok(())
                },
                TaskStateImpl::Finished => Ok(()), 
                TaskStateImpl::Error(err) => Err(err.clone())
            }
        }?;

        Ok(cache)
    }
}

#[async_trait::async_trait]
impl LeafDownloadTask for ChunkListTask {
    fn clone_as_leaf_task(&self) -> Box<dyn LeafDownloadTask> {
        Box::new(self.clone())
    }

    fn abs_group_path(&self) -> Option<String> {
        self.0.state.read().unwrap().abs_path.clone()
    }

    fn context(&self) -> &dyn DownloadContext {
        self.0.context.as_ref()
    }
}

#[async_trait::async_trait]
impl DownloadTask for ChunkListTask {
    fn clone_as_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Pending => DownloadTaskState::Downloading(0 ,0.0), 
            TaskStateImpl::Downloading(downloading) => DownloadTaskState::Downloading(downloading.history_speed.latest(), 0.0), 
            TaskStateImpl::Finished => DownloadTaskState::Finished, 
            TaskStateImpl::Error(err) => DownloadTaskState::Error(err.clone()), 
        }
    }

    fn control_state(&self) -> DownloadTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => DownloadTaskControlState::Normal, 
            ControlStateImpl::Canceled => DownloadTaskControlState::Canceled
        }
    }


    fn on_post_add_to_root(&self, abs_path: String) {
        self.0.state.write().unwrap().abs_path = Some(abs_path);
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                let cur_speed = downloading.cur_chunk.downloader.calc_speed(when);
                downloading.history_speed.update(Some(cur_speed), when);
                cur_speed
            }
            _ => 0,
        }
    }

    fn cur_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.history_speed.latest(), 
            _ => 0,
        }
    }

    fn history_speed(&self) -> u32 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.history_speed.average(), 
            _ => 0,
        }
    }

    fn cancel(&self) -> BuckyResult<DownloadTaskControlState> {
        let waiters = {
            let mut state = self.0.state.write().unwrap();
            let waiters = match &mut state.control_state {
                ControlStateImpl::Normal(waiters) => {
                    let waiters = Some(waiters.transfer());
                    state.control_state = ControlStateImpl::Canceled;
                    waiters
                }, 
                _ => None
            };

            match &state.task_state {
                TaskStateImpl::Downloading(_) => {
                    state.task_state = TaskStateImpl::Error(BuckyError::new(BuckyErrorCode::UserCanceled, "cancel invoked"));
                }, 
                _ => {}
            };

            waiters
        };

        if let Some(waiters) = waiters {
            waiters.wake();
        }

        Ok(DownloadTaskControlState::Canceled)
    }

    async fn wait_user_canceled(&self) -> BuckyError {
        let waiter = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.control_state {
                ControlStateImpl::Normal(waiters) => Some(waiters.new_waiter()), 
                _ => None
            }
        };
        
        if let Some(waiter) = waiter {
            let _ = StateWaiter::wait(waiter, || self.control_state()).await;
        } 

        BuckyError::new(BuckyErrorCode::UserCanceled, "")
    }
}


pub struct ChunkListTaskReader {
    offset: u64,
    task: ChunkListTask
} 

impl ChunkListTaskReader {
    fn new(task: ChunkListTask) -> Self {
        Self {
            offset: 0, 
            task
        }
    }
}

impl Drop for ChunkListTaskReader {
    fn drop(&mut self) {
        let _ = self.task.cancel();
    }
}

impl std::io::Seek for ChunkListTaskReader {
    fn seek(
        self: &mut Self,
        pos: SeekFrom,
    ) -> std::io::Result<u64> {
        let len = self.task.chunk_list().total_len();
        let new_offset = match pos {
            SeekFrom::Start(offset) => len.min(offset), 
            SeekFrom::Current(offset) => {
                let offset = (self.offset as i64) + offset;
                let offset = offset.max(0) as u64;
                len.min(offset)
            },
            SeekFrom::End(offset) => {
                let offset = (len as i64) + offset;
                let offset = offset.max(0) as u64;
                len.min(offset)
            }
        };
        if new_offset < self.offset {
            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "single directed stream"))
        } else {
            self.offset = new_offset;

            Ok(new_offset)
        }

       
    }
}


impl DownloadTaskSplitRead for ChunkListTaskReader {
    fn poll_split_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<Option<(ChunkCache, Range<usize>)>>> {
        let pined = self.get_mut();
        let ranges = pined.task.chunk_list().range_of(pined.offset..pined.offset + buffer.len() as u64);
        if ranges.is_empty() {
            return Poll::Ready(Ok(None));
        }
        if let DownloadTaskState::Error(err) = pined.task.state() {
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, BuckyError::new(err, ""))));
        } 
        let (index, range) = ranges[0].clone();

        let result = match pined.task.create_cache(index) {
            Ok(cache) => {
                let mut reader = DownloadTaskReader::new(cache, pined.task.clone_as_leaf_task());
                use std::{io::{Seek}};
                match reader.seek(SeekFrom::Start(range.start)) {
                    Ok(_) => {
                        let result = DownloadTaskSplitRead::poll_split_read(Pin::new(&mut reader), cx, &mut buffer[0..(range.end - range.start) as usize]);
                        if let Poll::Ready(result) = &result {
                            if let Some((_, r)) = result.as_ref().ok().and_then(|r| r.as_ref()) {
                                pined.offset += (r.end - r.start) as u64;
                            }
                        }
                        result
                    },
                    Err(err) => {
                        return Poll::Ready(Err(err));
                    }
                }
            }
            Err(err) => {
                return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, err)));
            }
        };
        
        for (index, _) in ranges.into_iter().skip(1) {
            let _ = pined.task.create_cache(index);
        }

        result
    }
}


impl async_std::io::Read for ChunkListTaskReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        self.poll_split_read(cx, buffer).map(|result| result.map(|r| if let Some((_, r)) = r {
            r.end - r.start
        } else {
            0
        }))
    }
}


impl ChunkListTask {
    pub fn reader(
        stack: WeakStack,  
        name: String,
        chunk_list: ChunkListDesc, 
        context: Box<dyn DownloadContext>
    ) -> (Self, ChunkListTaskReader) {
        let task = Self::new(stack, name, chunk_list, context);
        let reader = ChunkListTaskReader::new(task.clone());
        
        (task, reader)
    }
}