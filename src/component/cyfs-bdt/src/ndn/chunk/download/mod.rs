mod config;
mod double;

use std::{
    sync::{RwLock},
    collections::LinkedList,
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
};
use super::{
    //encode::ChunkDecoder, 
    storage::{ChunkReader}, 
    view::ChunkView, 
};
pub use config::ChunkDownloadConfig;

use double::DoubleSession;

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

enum SessionType {
    Single(DownloadSession), 
    Double(DoubleSession), 
    // Multi(MultiSesssion)
}

struct SessionsImpl {
    session_type: SessionType
}

#[derive(Clone)]
struct DownloadSessions(Arc<SessionsImpl>);

impl DownloadSessions {
    fn new(
        stack: WeakStack, 
        chunk: &ChunkId, 
        session_id: TempSeq, 
        config: Arc<ChunkDownloadConfig>,
        view: ChunkView) -> Self {
        if config.force_stream 
            || config.second_source.is_none() {
            let stack = Stack::from(&stack);
            let channel = stack.ndn().channel_manager().create_channel(&config.prefer_source);

            Self(Arc::new(SessionsImpl {
                session_type: SessionType::Single(
                    DownloadSession::new(
                        chunk.clone(), 
                        session_id, 
                        channel, 
                        PieceSessionType::Stream(0), 
                        view,
                        config.referer.clone()))
                }))
        } else if config.second_source.is_some() {
            let double_session = DoubleSession::new(stack, &view, session_id, &config);

            Self(Arc::new(SessionsImpl {
                session_type: SessionType::Double(double_session)
            }))
        } else {
            unimplemented!()
        }
    }

    async fn start(&self) -> TaskState {
        match &self.0.session_type {
            SessionType::Single(session) => {
                if let Ok(_) = session.channel().download(session.clone()) {
                    session.wait_finish().await
                } else {
                    unreachable!()
                }
            },
            SessionType::Double(double_session) => {
                double_session.start().await
            }
        }
    }

    fn take_chunk_content(&self) -> Option<Arc<Vec<u8>>> {
        match &self.0.session_type {
            SessionType::Single(session) => {
                session.take_chunk_content()
            },
            SessionType::Double(double_session) => {
                double_session.take_chunk_content()
            }
        }
    }

    fn get_redirect(&self) -> Option<(DeviceId, String)> {
        match &self.0.session_type {
            SessionType::Single(session) => {
                session.get_redirect()
            },
            SessionType::Double(session) => {
                session.get_redirect()
            }
        }
    }

}

struct InitState {
    view: ChunkView, 
    waiters: StateWaiter, 
}

struct RunningState {
    view: ChunkView, 
    sessions: DownloadSessions, 
    waiters: StateWaiter, 
}

enum StateImpl {
    Init(InitState), 
    Running(RunningState), 
    // Paused(DownloadSessions), 
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
    configs: RwLock<LinkedList<Arc<ChunkDownloadConfig>>>, 
    resource: ResourceManager
}

#[derive(Clone)]
pub struct ChunkDownloader(Arc<ChunkDowloaderImpl>);

// 不同于Uploader，Downloader可以被多个任务复用；
impl ChunkDownloader {
    pub fn new(
        view: ChunkView, 
        stack: WeakStack) -> Self {
        Self(Arc::new(ChunkDowloaderImpl {
            stack, 
            chunk: view.chunk().clone(), 
            state: RwLock::new(StateImpl::Init(InitState {
                view, 
                waiters: StateWaiter::new()})), 
            configs: RwLock::new(LinkedList::new()), 
            resource: ResourceManager::new(None)
        }))
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    // 直接返回finished
    pub fn finished(stack: WeakStack, view: &ChunkView, content: Arc<Box<dyn ChunkReader>>) -> Self {
        Self(Arc::new(ChunkDowloaderImpl {
            stack, 
            chunk: view.chunk().clone(), 
            state: RwLock::new(StateImpl::Finished(content)), 
            configs: RwLock::new(LinkedList::new()), 
            resource: ResourceManager::new(None)
        }))
    }

    pub fn add_config(
        &self, 
        config: Arc<ChunkDownloadConfig>
    ) -> BuckyResult<()> {
        // TODO：如果多个不同task传入不同的config，需要合并config中的源;
        // 并且合并resource manager
        self.0.configs.write().unwrap().push_back(config.clone());
        let stack = Stack::from(&self.0.stack);
        let view = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StateImpl::Init(init) => Some(init.view.clone()),
                _ => None 
            }
        };
        let sessions = view.and_then(|view| {
            let sessions = DownloadSessions::new(
                self.0.stack.clone(), 
                self.chunk(), 
                stack.ndn().chunk_manager().gen_session_id(), 
                config.clone(), 
                view);
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Init(init) => {
                    let mut running = RunningState {
                        view: init.view.clone(), 
                        sessions: sessions.clone(), 
                        waiters: StateWaiter::new()
                    };
                    std::mem::swap(&mut running.waiters, &mut init.waiters);
                    *state = StateImpl::Running(running);

                    Some(sessions)
                }, 
                _ => {
                    None
                }
            }
        });

        if let Some(sessions) = sessions { 
            let downloader = self.clone();
            let stack = stack.clone();
            let config = config.clone();
            task::spawn(async move {
                let waiters = {
                    match sessions.start().await {
                        TaskState::Finished => {
                            let mut waiters = StateWaiter::new();
                            let cache = sessions.take_chunk_content().unwrap();
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
                            match err {
                                BuckyErrorCode::Redirect | BuckyErrorCode::NotConnected => {
                                    if let Some((target_id, referer)) = sessions.get_redirect() {
                                        let mut config = ChunkDownloadConfig::force_stream(target_id.clone());
                                        config.referer = Some(referer);
    
                                        let _ = downloader.add_config(Arc::new(config));
                                        None
                                    } else {
                                        unreachable!()
                                    }
                                },
                                BuckyErrorCode::Pending => {
                                    let _ = async_std::future::timeout(stack.config().ndn.channel.wait_redirect_timeout, 
                                                                       async_std::future::pending::<()>());
                                    // restart session
                                    let _ = downloader.add_config(config);
                                    None
                                },
                                _ => {
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
                                }
                            }
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
