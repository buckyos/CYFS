use std::{
    sync::{RwLock},
};
use async_std::{
    sync::Arc, 
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
    storage::{ChunkReader}, 
};

struct DownloadingState {
    cache: ChunkCache, 
    session: Option<DownloadSession>
}

enum StateImpl {
    Loading, 
    Downloading(DownloadingState), 
    Finished
}



struct ChunkDowloaderImpl { 
    stack: WeakStack, 
    chunk: ChunkId, 
    context: MultiDownloadContext, 
    state: RwLock<StateImpl>, 
}

#[derive(Clone)]
pub struct ChunkDownloader(Arc<ChunkDowloaderImpl>);

// 不同于Uploader，Downloader可以被多个任务复用；
impl ChunkDownloader {
    pub fn new(
        stack: WeakStack, 
        chunk: ChunkId, 
        chunk_cache: ChunkCache,  
    ) -> Self {
        let downloader = Self(Arc::new(ChunkDowloaderImpl {
            stack: stack.clone(), 
            chunk, 
            state: RwLock::new(StateImpl::Loading), 
            context: MultiDownloadContext::new(), 
        }));

        {
            let downloader = downloader.clone();
            
            task::spawn(async move {
                let stack = Stack::from(&downloader.0.stack);
                
                match downloader.load(
                    stack.ndn().chunk_manager().store().clone_as_reader(), 
                    stack.ndn().chunk_manager().raw_caches()).await {
                    Ok(cache) => {
                        let _ = chunk_cache.stream().load(true, cache).unwrap();
                        let state = &mut *downloader.0.state.write().unwrap();
                        match &state {
                            StateImpl::Loading => {
                                *state = StateImpl::Finished;
                            },
                            _ => unreachable!()
                        }
                    },
                    Err(_err) => {
                        let state = &mut *downloader.0.state.write().unwrap();
                        match &state {
                            StateImpl::Loading => {
                                *state = StateImpl::Downloading(DownloadingState { 
                                    cache: chunk_cache, 
                                    session: None 
                                });
                            },
                            _ => unreachable!()
                        }
                    }
                }
            });
        }
        
        downloader
    }

    async fn load(&self, storage: Box<dyn ChunkReader>, raw_cache: &RawCacheManager) -> BuckyResult<Box<dyn RawCache>> {
        let reader = storage.get(self.chunk()).await?;

        let cache = raw_cache.alloc(self.chunk().len()).await;
        let writer = cache.async_writer().await?;

        let written = async_std::io::copy(reader, writer).await? as usize;
        
        if written != self.chunk().len() {
            Err(BuckyError::new(BuckyErrorCode::InvalidInput, ""))
        } else {
            Ok(cache)
        }
    }

    pub fn context(&self) -> &MultiDownloadContext {
        &self.0.context
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
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
        let (session, cache) = {
            match &*self.0.state.read().unwrap() {
                StateImpl::Downloading(downloading) => (downloading.session.clone(), Some(downloading.cache.clone())), 
                _ => (None, None)
            }
        };
        if let Some(session) = session {
            if !self.context().source_exists(session.source()) {
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
          
        if cache.is_none() {
            return 0;
        }

        let cache = cache.unwrap();
        let stack = Stack::from(&self.0.stack);
        let mut sources = self.context().sources_of(|_| true, 1);

        if sources.len() > 0 {
            if !cache.stream().loaded() {
                let raw_cache = stack.ndn().chunk_manager().raw_caches().alloc_mem(self.chunk().len());
                let _ = cache.stream().load(false, raw_cache);
            }
           
            let source = sources.pop_front().unwrap();
            let channel = stack.ndn().channel_manager().create_channel(&source.target).unwrap();
            
           
            let mut source: DownloadSourceWithReferer<DeviceId> = source.into();
            source.encode_desc = match &source.encode_desc {
                ChunkEncodeDesc::Unknown => ChunkEncodeDesc::Stream(None, None, None).fill_values(self.chunk()), 
                ChunkEncodeDesc::Stream(..) => source.encode_desc.fill_values(self.chunk()), 
                _ => unimplemented!()
            };

            match channel.download( 
                self.chunk().clone(), 
                source, 
                cache.stream().clone()
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
