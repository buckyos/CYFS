use std::{
    sync::RwLock, 
};
use async_std::{
    sync::Arc, 
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


enum TaskStateImpl {
    Downloading(ChunkCache),
    Error(BuckyErrorCode), 
    Finished,
}

enum ControlStateImpl {
    Normal(StateWaiter), 
    Canceled,
}

struct StateImpl {
    control_state: ControlStateImpl, 
    task_state: TaskStateImpl,
}

struct ChunkTaskImpl {
    stack: WeakStack, 
    chunk: ChunkId, 
    context: SingleDownloadContext, 
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
        context: SingleDownloadContext, 
    ) -> Self {
        let strong_stack = Stack::from(&stack);
        let cache = strong_stack.ndn().chunk_manager().create_cache(&chunk);
        cache.downloader().context().add_context(context.clone());
        
        Self(Arc::new(ChunkTaskImpl {
            stack, 
            chunk, 
            context, 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Downloading(cache.clone()), 
                control_state: ControlStateImpl::Normal(StateWaiter::new()),
            }),
        }))
    } 

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn context(&self) -> &SingleDownloadContext  {
        &self.0.context
    }

    pub fn reader(&self) -> DownloadTaskReader {
        let strong_stack = Stack::from(&self.0.stack);
        let cache = strong_stack.ndn().chunk_manager().create_cache(self.chunk());
        DownloadTaskReader::new(cache, self.clone_as_task())
    }
}

#[async_trait::async_trait]
impl DownloadTask for ChunkTask {
    fn context(&self) -> &SingleDownloadContext {
        &self.0.context
    }
    
    fn clone_as_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Downloading(_) => DownloadTaskState::Downloading(0, 0.0), 
            TaskStateImpl::Error(err) => DownloadTaskState::Error(*err), 
            TaskStateImpl::Finished => DownloadTaskState::Finished
        }
    }

    fn control_state(&self) -> DownloadTaskControlState {
        match &self.0.state.read().unwrap().control_state {
            ControlStateImpl::Normal(_) => DownloadTaskControlState::Normal, 
            ControlStateImpl::Canceled => DownloadTaskControlState::Canceled
        }
    }

    fn priority_score(&self) -> u8 {
        DownloadTaskPriority::Normal as u8
    }

    fn sub_task(&self, _path: &str) -> Option<Box<dyn DownloadTask>> {
        None
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        if let Some(cache) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
                _ => None
            }
        } {
            cache.downloader().calc_speed(when)
        } else {
            0
        }
    }

    fn cur_speed(&self) -> u32 {
        if let Some(cache) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
                _ => None
            }
        } {
            cache.downloader().cur_speed()
        } else {
            0
        }
    }

    fn history_speed(&self) -> u32 {
        if let Some(cache) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
                _ => None
            }
        } {
            cache.downloader().history_speed()
        } else {
            0
        }
    }

    fn drain_score(&self) -> i64 {
        if let Some(cache) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
                _ => None
            }
        } {
            cache.downloader().drain_score()
        } else {
            0
        }
    }

    fn on_drain(&self, expect_speed: u32) -> u32 {
        if let Some(cache) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
                _ => None
            }
        } {
            cache.downloader().on_drain(expect_speed)
        } else {
            0
        }
    }

    fn cancel(&self) -> BuckyResult<DownloadTaskControlState> {
        let (cache, waiters) = {
            let mut state = self.0.state.write().unwrap();
            let waiters = match &mut state.control_state {
                ControlStateImpl::Normal(waiters) => {
                    let waiters = Some(waiters.transfer());
                    state.control_state = ControlStateImpl::Canceled;
                    waiters
                }, 
                _ => None
            };

            let cache = match &state.task_state {
                TaskStateImpl::Downloading(cache) => {
                    let cache = Some(cache.clone());
                    state.task_state = TaskStateImpl::Error(BuckyErrorCode::UserCanceled);
                    cache
                }, 
                _ => None
            };

            (cache, waiters)
        };

        if let Some(waiters) = waiters {
            waiters.wake();
        }

        if let Some(cache) = cache {
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
