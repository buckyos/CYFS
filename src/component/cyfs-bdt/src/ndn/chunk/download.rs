use std::{
    sync::{RwLock},
};
use async_std::{
    sync::Arc, 
    task
};
use futures::future::AbortRegistration;
use cyfs_base::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}
};
use super::super::{ 
    scheduler::*, 
    channel::*, 
    download::*,
};
use super::{
    //encode::ChunkDecoder, 
    storage::{ChunkReader},
};

#[derive(Clone)]
pub struct CacheReader {
    pub cache: Arc<Vec<u8>>
}

#[async_trait::async_trait]
impl ChunkReader for CacheReader {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(self.clone())
    }

    async fn exists(&self, _chunk: &ChunkId) -> bool {
        true
    }

    async fn get(&self, _chunk: &ChunkId) -> BuckyResult<Arc<Vec<u8>>> {
        Ok(self.cache.clone())
    }
}


struct InitState {
    waiters: StateWaiter, 
}

struct RunningState {
    session: DownloadSession, 
    waiters: StateWaiter, 
}

enum StateImpl {
    Init(InitState), 
    Running(RunningState), 
    Canceled(BuckyErrorCode),
    Finished(Arc<Box<dyn ChunkReader>>)  
}

impl StateImpl {
    pub fn to_task_state(&self) -> TaskState {
        match self {
            Self::Init(_) => TaskState::Running(0), 
            Self::Running(_) => TaskState::Running(0), 
            Self::Canceled(err) => TaskState::Canceled(*err), 
            Self::Finished(_) => TaskState::Finished
        }
    }
}

struct ChunkDowloaderImpl {
    stack: WeakStack, 
    chunk: ChunkId, 
    state: RwLock<StateImpl>, 
    context: MultiDownloadContext, 
}

#[derive(Clone)]
pub struct ChunkDownloader(Arc<ChunkDowloaderImpl>);

// 不同于Uploader，Downloader可以被多个任务复用；
impl ChunkDownloader {
    pub fn new(
        stack: WeakStack,
        chunk: ChunkId,  
    ) -> Self {
        Self(Arc::new(ChunkDowloaderImpl {
            stack, 
            chunk, 
            state: RwLock::new(StateImpl::Init(InitState {
                waiters: StateWaiter::new()})), 
            context: MultiDownloadContext::new() 
        }))
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    // 直接返回finished
    pub fn finished(
        stack: WeakStack, 
        chunk: ChunkId, 
        content: Arc<Box<dyn ChunkReader>>
    ) -> Self {
        Self(Arc::new(ChunkDowloaderImpl {
            stack, 
            chunk, 
            state: RwLock::new(StateImpl::Finished(content)), 
            context: MultiDownloadContext::new() 
        }))
    }

    pub fn context(&self) -> &MultiDownloadContext {
        &self.0.context
    }

    pub fn on_drain(
        &self, 
    ) -> BuckyResult<()> {
        // TODO：如果多个不同task传入不同的config，需要合并config中的源;
        // 并且合并resource manager
        let strong_stack = Stack::from(&self.0.stack);
        let session = {
            let mut sources = self.context().sources_of(|source| {
                if source.object_id.is_none() || source.object_id.as_ref().unwrap() == self.chunk().as_object_id() {
                    true
                } else {
                    false
                }
            }, 1);

            if sources.len() > 0 {
                let source = sources.pop_front().unwrap();
                let channel = strong_stack.ndn().channel_manager().create_channel(&source.target);

                let session = DownloadSession::new(
                    self.0.stack.clone(), 
                    self.chunk().clone(), 
                    strong_stack.ndn().chunk_manager().gen_session_id(), 
                    channel, 
                    PieceSessionType::Stream(0), 
                    source.referer, 
                    ResourceManager::new(None));

                let state = &mut *self.0.state.write().unwrap();
                match state {
                    StateImpl::Init(init) => {
                        
                        let mut running = RunningState {
                            session: session.clone(), 
                            waiters: StateWaiter::new()
                        };
                        std::mem::swap(&mut running.waiters, &mut init.waiters);
                        *state = StateImpl::Running(running);

                        Some(session)
                    }, 
                    _ => {
                        None
                    }
                }
            } else {
                None
            }
            
        };

        if let Some(session) = session { 
            let downloader = self.clone();
            task::spawn(async move {
                let waiters = {
                    let _ = session.channel().download(session.clone());
                    match session.wait_finish().await {
                        TaskState::Finished => {
                            let mut waiters = StateWaiter::new();
                            let cache = session.take_chunk_content().unwrap();
                            let state = &mut *downloader.0.state.write().unwrap();
                            match state {
                                StateImpl::Running(running) => {
                                    std::mem::swap(&mut waiters, &mut running.waiters);
                                    *state = StateImpl::Finished(Arc::new(Box::new(CacheReader {
                                        cache
                                    })));
                                },
                                StateImpl::Finished(_) => {
                                    
                                },
                                _ => unreachable!()
                            }
                            Some(waiters)
                        }, 
                        TaskState::Canceled(err) => {
                            let mut waiters = StateWaiter::new();
                            let state = &mut *downloader.0.state.write().unwrap();
                            match state {
                                StateImpl::Running(running) => {
                                    std::mem::swap(&mut waiters, &mut running.waiters);
                                    *state = StateImpl::Canceled(err);
                                },
                                _ => unreachable!()
                            }
                            Some(waiters)
                        }, 
                        _ => unreachable!()
                    }
                };
                if let Some(waiters) = waiters {
                    waiters.wake();
                }
            });
        }
        Ok(())
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub async fn wait_finish(&self) -> TaskState {
        enum NextStep {
            Wait(AbortRegistration), 
            Return(TaskState)
        }
        let next_step = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Init(init) => NextStep::Wait(init.waiters.new_waiter()), 
                StateImpl::Running(running) => NextStep::Wait(running.waiters.new_waiter()), 
                StateImpl::Finished(_) => NextStep::Return(TaskState::Finished), 
                StateImpl::Canceled(err) => NextStep::Return(TaskState::Canceled(*err)),
            }
        };
        match next_step {
            NextStep::Wait(waiter) => {
                StateWaiter::wait(waiter, | | self.schedule_state()).await
            }, 
            NextStep::Return(state) => state
        }
    }

    pub fn reader(&self) -> Option<Arc<Box<dyn ChunkReader>>> {
        let state = &*self.0.state.read().unwrap();
        match state {
            StateImpl::Finished(reader) => Some(reader.clone()), 
            _ => None
        }
    } 

    pub fn schedule_state(&self) -> TaskState {
        self.0.state.read().unwrap().to_task_state()
    }
}
