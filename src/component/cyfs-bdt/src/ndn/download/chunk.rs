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
    Init, 
    Pending, 
    Downloading(ChunkDownloader),
    Error(BuckyErrorCode), 
    Writting,  
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
    writers: Vec<Box <dyn ChunkWriterExt>>,
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
        writers: Vec<Box <dyn ChunkWriter>>, 
    ) -> Self {
        Self(Arc::new(ChunkTaskImpl {
            stack, 
            chunk, 
            range: None, 
            context, 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Init, 
                control_state: DownloadTaskControlState::Normal,
            }), 
            writers: writers.into_iter().map(|w| ChunkWriterExtWrapper::new(w).clone_as_writer()).collect(),
        }))
    } 

    pub fn with_range(
        stack: WeakStack, 
        chunk: ChunkId, 
        range: Option<Range<u64>>, 
        context: SingleDownloadContext, 
        writers: Vec<Box <dyn ChunkWriterExt>>, 
    ) -> Self {
        Self(Arc::new(ChunkTaskImpl {
            stack, 
            chunk, 
            range, 
            context, 
            state: RwLock::new(StateImpl {
                task_state: TaskStateImpl::Init, 
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

    async fn sync_chunk_state(&self) {
        loop {
            let downloader = {
                match &self.0.state.read().unwrap().task_state {
                    TaskStateImpl::Downloading(downloader) => downloader.clone(), 
                    _ => unreachable!()
                }
            };

            match downloader.wait_finish().await {
                DownloadTaskState::Finished => {
                    match downloader.reader().unwrap().get(self.chunk()).await {
                        Ok(content) => {
                            self.0.state.write().unwrap().task_state = TaskStateImpl::Writting;
                            for writer in &self.0.writers {
                                let _ = writer.write(self.chunk(), content.clone(), self.range()).await;
                                let _ = writer.finish().await;
                            }
                            let mut state = self.0.state.write().unwrap();
                            info!("{} finished", self);
                            state.task_state = TaskStateImpl::Finished;
                            break; 
                        }, 
                        Err(_err) => {
                            let stack = Stack::from(&self.0.stack);
                            let downloader = stack.ndn().chunk_manager().start_download(
                                self.chunk().clone(), 
                                self.context().clone(), 
                            ).await.unwrap();
                            info!("{} reset downloader for read chunk failed", self);
                            let mut state = self.0.state.write().unwrap();
                            state.task_state = TaskStateImpl::Downloading(downloader.clone());
                        }
                    }
                    
                }, 
                DownloadTaskState::Error(err) => {
                    error!("{} canceled", self);
                    self.0.state.write().unwrap().task_state = TaskStateImpl::Error(err);
                    for writer in &self.0.writers {
                        let _ = writer.err(err).await;
                    }

                    break; 
                }, 
                _ => unimplemented!()
            }
        }
    }
}

impl DownloadTask for ChunkTask {
    fn clone_as_task(&self) -> Box<dyn DownloadTask> {
        Box::new(self.clone())
    }

    fn state(&self) -> DownloadTaskState {
        match &self.0.state.read().unwrap().task_state {
            TaskStateImpl::Init => DownloadTaskState::Pending, 
            TaskStateImpl::Pending => DownloadTaskState::Pending, 
            TaskStateImpl::Downloading(downloader) => downloader.state(), 
            TaskStateImpl::Error(err) => DownloadTaskState::Error(*err), 
            TaskStateImpl::Writting => DownloadTaskState::Downloading(0, 100.0), 
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
        if let Some(downloader) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloader) => Some(downloader.clone()), 
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
                TaskStateImpl::Downloading(downloader) => Some(downloader.clone()), 
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
                TaskStateImpl::Downloading(downloader) => Some(downloader.clone()), 
                _ => None
            }
        } {
            downloader.history_speed()
        } else {
            0
        }
    }

    fn drain_score(&self) -> i64 {
        if let Some(downloader) = {
            let state = self.0.state.read().unwrap();
            match &state.task_state {
                TaskStateImpl::Downloading(downloader) => Some(downloader.clone()), 
                _ => None
            }
        } {
            downloader.drain_score()
        } else {
            0
        }
    }

    fn on_drain(&self, expect_speed: u32) -> u32 {
        if let Some(downloader) = {
            let mut state = self.0.state.write().unwrap();
            match &state.task_state {
                TaskStateImpl::Init => {
                    info!("{} started", self);
                    let stack = Stack::from(&self.0.stack);
                    state.task_state = TaskStateImpl::Pending;
                    let task = self.clone();
                    task::spawn(async move {
                        let downloader = stack.ndn().chunk_manager().start_download(
                            task.chunk().clone(), 
                            task.context().clone(), 
                        ).await.unwrap();
                        {
                            let mut state = task.0.state.write().unwrap();
                            match &state.task_state {
                                TaskStateImpl::Pending => {
                                    state.task_state = TaskStateImpl::Downloading(downloader.clone())
                                }, 
                                _ => unreachable!()
                            }
                        }
                        task.sync_chunk_state().await;
                    });
                    None
                }, 
                TaskStateImpl::Downloading(downloader) => Some(downloader.clone()), 
                _ => None
            }
        } {
            downloader.on_drain(expect_speed)
        } else {
            0
        }
    }
}
