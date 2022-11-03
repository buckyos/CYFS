use std::{
    sync::{RwLock},
    io::SeekFrom, 
};
use async_std::{
    sync::Arc, 
    pin::Pin, 
    task::{Context, Poll},
    task
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

struct DownloadingState { 
    chunks: Vec<ChunkCache>, 
    history_speed: HistorySpeed,
    drain_score: i64
}

enum ControlStateImpl {
    Normal(StateWaiter), 
    Canceled,
}

enum TaskStateImpl {
    Downloading(DownloadingState), 
    Error(BuckyErrorCode), 
    Finished
}

struct StateImpl {
    control_state: ControlStateImpl, 
    task_state: TaskStateImpl,
}

struct TaskImpl {
    stack: WeakStack, 
    file: File,
    chunk_list: ChunkListDesc, 
    context: SingleDownloadContext, 
    state: RwLock<StateImpl>,  
}

#[derive(Clone)]
pub struct FileTask(Arc<TaskImpl>);

impl std::fmt::Display for FileTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileTask::{{file:{}}}", self.file().desc().file_id())
    }
}

impl FileTask {
    pub fn new(
        stack: WeakStack,  
        file: File,  
        chunk_list: Option<ChunkListDesc>, 
        context: SingleDownloadContext, 
    ) -> Self {
        let chunk_list = chunk_list.unwrap_or(ChunkListDesc::from_file(&file).unwrap());
        let strong_stack = Stack::from(&stack);
        let task = Self(Arc::new(TaskImpl {
            stack, 
            file: file.clone(), 
            chunk_list, 
            context, 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Downloading(DownloadingState {
                    drain_score: 0, 
                    chunks: vec![], 
                    history_speed: HistorySpeed::new(0, strong_stack.config().ndn.channel.history_speed.clone())
                }),
                control_state: ControlStateImpl::Normal(StateWaiter::new()),
            }), 
        }));

        {
            let task = task.clone();
            task::spawn(async move {
                let _ = task.begin().await;
            });
        }
        task
    } 

    pub fn file(&self) -> &File {
        &self.0.file
    }

    pub fn chunk_list(&self) -> &ChunkListDesc {
        &self.0.chunk_list
    }

    pub fn context(&self) -> &SingleDownloadContext {
        &self.0.context
    }

    pub fn reader(&self) -> FileTaskReader {
        FileTaskReader::new(self.clone())
    }

    async fn begin(&self) {
        let stack = Stack::from(&self.0.stack);

        for chunk in self.chunk_list().chunks() {
            let cache = stack.ndn().chunk_manager().create_cache(chunk);

            if cache.wait_exists(0..chunk.len(), || self.wait_user_canceled()).await.is_err() {
                return;
            }
        }

        let mut state = self.0.state.write().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(_) => {
                state.task_state = TaskStateImpl::Finished
            },
            _ => {}
        }
    }

    fn create_cache(&self, index: usize) -> BuckyResult<ChunkCache> {
        let stack = Stack::from(&self.0.stack);
        let chunk = self.chunk_list().chunks()[index];
        let cache = stack.ndn().chunk_manager().create_cache(&chunk);
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                if downloading.chunks.iter().find(|cache| cache.chunk().eq(&chunk)).is_none() {
                    cache.downloader().context().add_context(self.context().clone());
                    downloading.chunks.push(cache.clone());
                } 
                Ok(cache)
            },
            TaskStateImpl::Finished => Ok(cache), 
            TaskStateImpl::Error(err) => Err(BuckyError::new(*err, ""))
        }
    }
}

#[async_trait::async_trait]
impl DownloadTask for FileTask {
    fn context(&self) -> &SingleDownloadContext {
        &self.0.context
    }

    fn clone_as_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Downloading(_) => DownloadTaskState::Downloading(0 ,0.0), 
            TaskStateImpl::Finished => DownloadTaskState::Finished, 
            TaskStateImpl::Error(err) => DownloadTaskState::Error(*err), 
        }
    }

    fn control_state(&self) -> DownloadTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => DownloadTaskControlState::Normal, 
            ControlStateImpl::Canceled => DownloadTaskControlState::Canceled
        }
    }


    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut state = self.0.state.write().unwrap();
        match &mut state.task_state {
            TaskStateImpl::Downloading(downloading) => {
                let mut cur_speed = 0;
                for cache in &downloading.chunks {
                    cur_speed += cache.downloader().calc_speed(when);
                }
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

    fn drain_score(&self) -> i64 {
        let state = self.0.state.read().unwrap();
        match &state.task_state {
            TaskStateImpl::Downloading(downloading) => downloading.drain_score, 
            _ => 0,
        }
    }

    fn on_drain(&self, expect_speed: u32) -> u32 {
        let chunks: Vec<ChunkCache> = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => downloading.chunks.clone(),
                _ => vec![]
            }
        };

        let mut new_expect = 0;
        if chunks.len() > 0 {
            for cache in chunks {
                new_expect += cache.downloader().on_drain(expect_speed / chunks.len() as u32);
            }
        }
       

        {
            let mut state = self.0.state.write().unwrap();
            match &mut state.task_state {
                TaskStateImpl::Downloading(downloading) => {
                    downloading.drain_score += new_expect as i64 - expect_speed as i64;
                    downloading.chunks.sort_by(|l, r| r.downloader().drain_score().cmp(&l.downloader().drain_score()));
                },
                _ => {}
            }
        }

        new_expect

    }


    fn cancel(&self) -> BuckyResult<DownloadTaskControlState> {
        let (chunks, waiters) = {
            let mut state = self.0.state.write().unwrap();
            let waiters = match &mut state.control_state {
                ControlStateImpl::Normal(waiters) => {
                    let waiters = Some(waiters.transfer());
                    state.control_state = ControlStateImpl::Canceled;
                    waiters
                }, 
                _ => None
            };

            let chunks = match &state.task_state {
                TaskStateImpl::Downloading(downloading) => {
                    let chunks = downloading.chunks.clone();
                    state.task_state = TaskStateImpl::Error(BuckyErrorCode::UserCanceled);
                    chunks
                }, 
                _ => vec![]
            };

            (chunks, waiters)
        };

        if let Some(waiters) = waiters {
            waiters.wake();
        }

        for cache in chunks {
            cache.downloader().context().remove_context(self.context(), self.state());
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


pub struct FileTaskReader {
    offset: u64,
    task: FileTask
} 

impl FileTaskReader {
    fn new(task: FileTask) -> Self {
        Self {
            offset: 0, 
            task
        }
    }
}

impl std::io::Seek for FileTaskReader {
    fn seek(
        self: &mut Self,
        pos: SeekFrom,
    ) -> std::io::Result<u64> {
        let len = self.task.file().len();
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
        self.offset = new_offset;

        Ok(new_offset)
    }
}

impl async_std::io::Read for FileTaskReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let pined = self.get_mut();
        let ranges = pined.task.chunk_list().range_of(pined.offset..pined.offset + buffer.len() as u64);
        if ranges.is_empty() {
            return Poll::Ready(Ok(0));
        }
        if let DownloadTaskState::Error(err) = pined.task.state() {
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, BuckyError::new(err, ""))));
        } 
        let (index, range) = ranges[0].clone();

        let result = match pined.task.create_cache(index) {
            Ok(cache) => {
                let mut reader = DownloadTaskReader::new(cache, pined.task.clone_as_task());
                use std::{io::{Seek}};
                match reader.seek(SeekFrom::Start(range.start)) {
                    Ok(_) => {
                        let result = async_std::io::Read::poll_read(Pin::new(&mut reader), cx, &mut buffer[0..(range.end - range.start) as usize]);
                        if let Poll::Ready(result) = &result {
                            if let Ok(len) = result {
                                pined.offset += *len as u64;
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