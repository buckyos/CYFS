use std::{
    sync::RwLock, 
    ops::Range
};
use async_std::{
    sync::Arc, 
    task
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
    Downloading(cache2::ChunkCache), 
    Error(BuckyErrorCode), 
    Finished,
}

struct StateImpl {
    control_state: DownloadTaskControlState, 
    task_state: TaskStateImpl,
}

struct ChunkTaskImpl {
    stack: WeakStack, 
    chunk: ChunkId, 
    range: Option<Range<u64>>, 
    context: SingleDownloadContext, 
    state: RwLock<StateImpl>,  
    writers: Vec<Box <dyn ChunkWriter2>>,
}

#[derive(Clone)]
pub struct ChunkTask(Arc<ChunkTaskImpl>);


impl std::fmt::Display for ChunkTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkTask{{chunk:{}, range:{:?}}}", self.chunk(), self.range())
    }
}

impl ChunkTask {
    pub fn new(
        stack: WeakStack, 
        chunk: ChunkId, 
        context: SingleDownloadContext, 
        writers: Vec<Box <dyn ChunkWriter2>>, 
    ) -> Self {
        let strong_stack = Stack::from(&stack);
        let cache = strong_stack.ndn().chunk_manager2().create_cache(&chunk, Some(context.clone()));
        Self(Arc::new(ChunkTaskImpl {
            stack, 
            chunk, 
            range: None, 
            context, 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Downloading(cache), 
                control_state: DownloadTaskControlState::Normal,
            }), 
            writers
        }))
    } 

    pub fn with_range(
        stack: WeakStack, 
        chunk: ChunkId, 
        range: Option<Range<u64>>, 
        context: SingleDownloadContext, 
        writers: Vec<Box <dyn ChunkWriter2>>, 
    ) -> Self {
        let strong_stack = Stack::from(&stack);
        let cache = strong_stack.ndn().chunk_manager2().create_cache(&chunk, Some(context.clone()));
        Self(Arc::new(ChunkTaskImpl {
            stack, 
            chunk, 
            range, 
            context, 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Downloading(cache), 
                control_state: DownloadTaskControlState::Normal,
            }), 
            writers,
        }))
    } 

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn range(&self) -> Option<Range<u64>> {
        self.0.range.clone()
    }

    pub fn context(&self) -> &SingleDownloadContext  {
        &self.0.context
    }
}

impl DownloadTask for ChunkTask {
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
        self.0.state.read().unwrap().control_state.clone()
    }

    fn priority_score(&self) -> u8 {
        DownloadTaskPriority::Normal as u8
    }

    fn sub_task(&self, _path: &str) -> Option<Box<dyn DownloadTask>> {
        None
    }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        // if let Some(cache) = {
        //     let state = self.0.state.read().unwrap();
        //     match &state.task_state {
        //         TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
        //         _ => None
        //     }
        // } {
        //     cache.calc_speed(when)
        // } else {
        //     0
        // }
        unimplemented!()
    }

    fn cur_speed(&self) -> u32 {
        // if let Some(cache) = {
        //     let state = self.0.state.read().unwrap();
        //     match &state.task_state {
        //         TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
        //         _ => None
        //     }
        // } {
        //     cache.cur_speed()
        // } else {
        //     0
        // }
        unimplemented!()
    }

    fn history_speed(&self) -> u32 {
        // if let Some(cache) = {
        //     let state = self.0.state.read().unwrap();
        //     match &state.task_state {
        //         TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
        //         _ => None
        //     }
        // } {
        //     cache.history_speed()
        // } else {
        //     0
        // }
        unimplemented!()
    }

    fn drain_score(&self) -> i64 {
        // if let Some(cache) = {
        //     let state = self.0.state.read().unwrap();
        //     match &state.task_state {
        //         TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
        //         _ => None
        //     }
        // } {
        //     cache.drain_score()
        // } else {
        //     0
        // }
        unimplemented!()
    }

    fn on_drain(&self, expect_speed: u32) -> u32 {
        // if let Some(cache) = {
        //     let mut state = self.0.state.write().unwrap();
        //     match &state.task_state {
        //         TaskStateImpl::Downloading(cache) => Some(cache.clone()), 
        //         _ => None
        //     }
        // } {
        //     cache.on_drain(expect_speed)
        // } else {
        //     0
        // }
        unimplemented!()
    }
}
