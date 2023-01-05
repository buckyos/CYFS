use std::{
    sync::{RwLock, Arc, Weak},
};
use async_std::{ 
    task, 
};
use cyfs_base::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}
};
use super::super::{ 
    types::*, 
    channel::*, 
    download::*,
};
use super::{
    cache::*, 
};

struct DownloadingState {
    session: Option<DownloadSession>
}

enum StateImpl {
    Loading, 
    Downloading(DownloadingState), 
    Finished
}

struct ChunkDowloaderImpl { 
    stack: WeakStack, 
    task: Box<dyn DownloadTask>, 
    context: Box<dyn DownloadContext>, 
    cache: ChunkCache, 
    state: RwLock<StateImpl>, 
}

#[derive(Clone)]
pub struct ChunkDownloader(Arc<ChunkDowloaderImpl>);

pub struct WeakChunkDownloader(Weak<ChunkDowloaderImpl>);

impl WeakChunkDownloader {
    pub fn to_strong(&self) -> Option<ChunkDownloader> {
        Weak::upgrade(&self.0).map(|arc| ChunkDownloader(arc))
    }
}

impl ChunkDownloader {
    pub fn to_weak(&self) -> WeakChunkDownloader {
        WeakChunkDownloader(Arc::downgrade(&self.0))
    }
}


impl ChunkDownloader {
    pub fn new(
        stack: WeakStack, 
        cache: ChunkCache, 
        task: Box<dyn DownloadTask>, 
        context: Box<dyn DownloadContext>
    ) -> Self {
        let downloader = Self(Arc::new(ChunkDowloaderImpl {
            stack, 
            cache, 
            task, 
            context, 
            state: RwLock::new(StateImpl::Loading), 
        }));

        {
            let downloader = downloader.clone();
            
            task::spawn(async move {
                let finished = downloader.cache().wait_loaded().await;
                {   
                    let state = &mut *downloader.0.state.write().unwrap();
                    if let StateImpl::Loading = state {
                        if finished {
                            *state = StateImpl::Finished;
                        } else {
                            *state = StateImpl::Downloading(DownloadingState { 
                                session: None 
                            });
                        }
                    } else {
                        unreachable!()
                    }
                }
               
                if !finished {
                    downloader.on_drain(0);
                }
                
            });
        }
        
        downloader
    }

    pub fn owner(&self) -> &dyn DownloadTask {
        self.0.task.as_ref()
    }

    pub fn context(&self) -> &dyn DownloadContext {
        self.0.context.as_ref()
    }

    pub fn cache(&self) -> &ChunkCache {
        &self.0.cache
    }

    pub fn chunk(&self) -> &ChunkId {
        self.cache().chunk()
    }

    pub fn calc_speed(&self, when: Timestamp) -> u32 {
        if let Some(session) = {
            match &*self.0.state.read().unwrap() {
                StateImpl::Downloading(downloading) => downloading.session.clone(), 
                _ => None
            }
        } {
            session.calc_speed(when)
        } else {
            0
        }
    } 

    pub fn cur_speed(&self) -> u32 {
        if let Some(session) = {
            match &*self.0.state.read().unwrap() {
                StateImpl::Downloading(downloading) => downloading.session.clone(), 
                _ => None
            }
        } {
            session.cur_speed()
        } else {
            0
        }
    }

    pub fn history_speed(&self) -> u32 {
        if let Some(session) = {
            match &*self.0.state.read().unwrap() {
                StateImpl::Downloading(downloading) => downloading.session.clone(), 
                _ => None
            }
        } {
            session.history_speed()
        } else {
            0
        }
    }

    pub fn drain_score(&self) -> i64 {
        0
    }

    pub fn on_drain(&self, _: u32) -> u32 {
        let (session, start) = {
            match &*self.0.state.read().unwrap() {
                StateImpl::Downloading(downloading) => (downloading.session.clone(), true), 
                _ => (None, false)
            }
        };
        if let Some(session) = session {
            if !task::block_on(self.context().source_exists(session.source())) {
                session.cancel_by_error(BuckyError::new(BuckyErrorCode::UserCanceled, "user canceled"));
                let state = &mut *self.0.state.write().unwrap();
                match state {
                    StateImpl::Downloading(downloading) => {
                        if let Some(exists) = downloading.session.clone() {
                            if exists.ptr_eq(&session) {
                                // info!("{} cancel session {}", self, session);
                                downloading.session = None;
                            }
                        }
                    }, 
                    _ => {}
                }
            } else {
                return session.cur_speed();
            }
        } 
          
        if !start {
            return 0;
        }

        let cache = &self.0.cache;
        let stack = Stack::from(&self.0.stack);
        let mut sources = task::block_on(self.context().sources_of(&DownloadSourceFilter::default(), 1));

        if sources.len() > 0 { 
            let source = sources.pop_front().unwrap();
            let channel = stack.ndn().channel_manager().create_channel(&source.target).unwrap();
            
            let mut source: DownloadSource<DeviceId> = source.into();
            source.codec_desc = match &source.codec_desc {
                ChunkCodecDesc::Unknown => ChunkCodecDesc::Stream(None, None, None).fill_values(self.chunk()), 
                ChunkCodecDesc::Stream(..) => source.codec_desc.fill_values(self.chunk()), 
                _ => unimplemented!()
            };

            match channel.download( 
                self.chunk().clone(), 
                source, 
                cache.stream().clone(), 
                Some(self.context().referer().to_owned()), 
                self.owner().abs_group_path().clone()
            ) {
                Ok(session) => {
                    let (start, exists) = {
                        let state = &mut *self.0.state.write().unwrap();
                        match state {
                            StateImpl::Downloading(downloading) => {
                                if let Some(exists) = &downloading.session {
                                    (false, Some(exists.clone()))
                                } else {
                                    downloading.session = Some(session.clone());
                                    (true, None)
                                }
                            }, 
                            _ => (false, None)
                        }
                    };
                    if start {
                        session.start();
                        session.cur_speed()
                    } else if let Some(session) = exists {
                        session.cur_speed()
                    } else {
                        0
                    }
                }, 
                Err(_) => {
                    if let Some(session) = {
                        match &*self.0.state.read().unwrap() {
                            StateImpl::Downloading(downloading) => downloading.session.clone(), 
                            _ => None
                        }
                    } {
                        session.cur_speed()
                    } else {
                        0
                    }
                }
            }
        } else {
            0
        }
    }
}
