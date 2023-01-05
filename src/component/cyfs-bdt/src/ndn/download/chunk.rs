use std::{
    sync::RwLock, 
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
    stack::{WeakStack, Stack}
};
use super::super::{
    chunk::*, 
};
use super::{
    common::*
};


struct DownloadingState {
    downloader: ChunkDownloader,
}

enum TaskStateImpl {
    Init, 
    Downloading(DownloadingState),
    Error(BuckyError), 
    Finished(ChunkCache),
}

enum ControlStateImpl {
    Normal(StateWaiter), 
    Canceled,
}

struct StateImpl {
    abs_path: Option<String>, 
    control_state: ControlStateImpl, 
    task_state: TaskStateImpl,
}

struct ChunkTaskImpl {
    stack: WeakStack, 
    chunk: ChunkId, 
    context: Box<dyn DownloadContext>, 
    state: RwLock<StateImpl>,  
}

#[derive(Clone)]
pub struct ChunkTask(Arc<ChunkTaskImpl>);

impl std::fmt::Display for ChunkTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkTask{{chunk:{}}}", self.chunk())
    }
}

impl ChunkTask {
    pub fn new(
        stack: WeakStack, 
        chunk: ChunkId, 
        context: Box<dyn DownloadContext>, 
    ) -> Self {
        Self(Arc::new(ChunkTaskImpl { 
            stack, 
            chunk, 
            context, 
            state: RwLock::new(StateImpl {
                abs_path: None, 
                task_state: TaskStateImpl::Init, 
                control_state: ControlStateImpl::Normal(StateWaiter::new()),
            }),
        }))
    } 

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }
}


#[async_trait::async_trait]
impl LeafDownloadTask for ChunkTask {
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
impl DownloadTask for ChunkTask {
    fn clone_as_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Init => DownloadTaskState::Downloading(0, 0.0), 
            TaskStateImpl::Downloading(downloading) => DownloadTaskState::Downloading(downloading.downloader.cur_speed(), 0.0), 
            TaskStateImpl::Error(err) => DownloadTaskState::Error(err.clone()), 
            TaskStateImpl::Finished(_) => DownloadTaskState::Finished
        }
    }

    fn control_state(&self) -> DownloadTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => DownloadTaskControlState::Normal, 
            ControlStateImpl::Canceled => DownloadTaskControlState::Canceled
        }
    }

    fn on_post_add_to_root(&self, abs_path: String) {
        let stack = Stack::from(&self.0.stack);
        let downloader = stack.ndn().chunk_manager().create_downloader(self.chunk(), self.clone_as_leaf_task());

        let mut state = self.0.state.write().unwrap();
        state.abs_path = Some(abs_path);
        match &state.task_state {
            TaskStateImpl::Init => {
                state.task_state = TaskStateImpl::Downloading(DownloadingState {
                    downloader, 
                });
            }, 
            _ => {}
        }
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        if let Some(downloader) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => Some(downloading.downloader.clone()), 
                _ => None
            }
        } {
            downloader.calc_speed(when)
        } else {
            0
        }
    }

    fn cur_speed(&self) -> u32 {
        if let Some(downloader) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => Some(downloading.downloader.clone()), 
                _ => None
            }
        } {
            downloader.cur_speed()
        } else {
            0
        }
    }

    fn history_speed(&self) -> u32 {
        if let Some(downloader) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloading) => Some(downloading.downloader.clone()), 
                _ => None
            }
        } {
            downloader.history_speed()
        } else {
            0
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


pub struct ChunkTaskReader(DownloadTaskReader);

impl Drop for ChunkTaskReader {
    fn drop(&mut self) {
        let _ = self.0.task().cancel();
    }
}

impl DownloadTaskSplitRead for ChunkTaskReader {
    fn poll_split_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<Option<(ChunkCache, Range<usize>)>>> {
        DownloadTaskSplitRead::poll_split_read(Pin::new(&mut self.get_mut().0), cx, buffer)
    }
}

impl std::io::Seek for ChunkTaskReader {
    fn seek(
        self: &mut Self,
        pos: SeekFrom,
    ) -> std::io::Result<u64> {
        std::io::Seek::seek(&mut self.0, pos)
    }
}

impl async_std::io::Read for ChunkTaskReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        async_std::io::Read::poll_read(Pin::new(&mut self.get_mut().0), cx, buffer)
    }
}
impl ChunkTask {
    pub fn reader(
        stack: WeakStack, 
        chunk: ChunkId, 
        context: Box<dyn DownloadContext>, 
    ) -> (Self, ChunkTaskReader) {
        let strong_stack = Stack::from(&stack);

        let task = Self::new(stack, chunk, context);

        let cache = strong_stack.ndn().chunk_manager().create_cache(task.chunk());

        let reader = ChunkTaskReader(DownloadTaskReader::new(cache, task.clone_as_leaf_task()));

        (task, reader)
    }
}