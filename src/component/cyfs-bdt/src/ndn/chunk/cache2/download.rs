use std::{
    sync::{RwLock},
};
use async_std::{
    sync::Arc, 
    task, 
    io::prelude::*
};
use cyfs_base::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}
};
use super::super::super::{ 
    types::*, 
    channel::{*, protocol::v0::*}, 
    download::*,
};
use super::super::{
    //encode::ChunkDecoder, 
    storage::{ChunkReader}, 
};
use super::{
    raw_cache::*,  
    stream::*
};


struct DownloadingState {
    cache: ChunkStreamCache, 
    session: Option<DownloadSession2>
}

enum StateImpl {
    Loading, 
    Downloading(DownloadingState), 
    Finished
}

impl StateImpl {
    pub fn to_task_state(&self) -> DownloadTaskState {
        match self {
            Self::Loading => DownloadTaskState::Downloading(0, 0.0), 
            Self::Downloading(_) => DownloadTaskState::Downloading(0, 0.0), 
            Self::Finished => DownloadTaskState::Finished
        }
    }
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
        stream_cache: ChunkStreamCache,  
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
                    stack.ndn().chunk_manager2().store().clone_as_reader(), 
                    stack.ndn().chunk_manager2().raw_caches()).await {
                    Ok(cache) => {
                        stream_cache.load(true, cache);
                        let state = &mut *downloader.0.state.write().unwrap();
                        match &state {
                            StateImpl::Loading => {
                                *state = StateImpl::Finished;
                            },
                            _ => unreachable!()
                        }
                    },
                    Err(_err) => {
                        downloader.try_download(stream_cache);
                    }
                }
            });
        }
        
        downloader
    }

    async fn load(&self, storage: Box<dyn ChunkReader>, raw_cache: &RawCacheManager) -> BuckyResult<Box<dyn RawCache>> {
        let mut reader = storage.read(self.chunk()).await?;

        let cache = raw_cache.alloc(self.chunk().len()).await;
        let mut writer = cache.async_writer().await?;
        let range_size = PieceData::max_payload();  

        let (_, end, step) = ChunkEncodeDesc::Stream(None, None, None).fill(self.chunk()).unwrap_as_stream();
        let mut buffer = vec![0u8; step as usize];

        use async_std::io::prelude::*;
        for index in 0..end {
            let (_, range) = PieceDesc::Range(index, step as u16).stream_piece_range(self.chunk());
            let len = reader.read(&mut buffer[..]).await?;
            if len != (range.end - range.start) as usize {
                return Err(BuckyError::new(BuckyErrorCode::InvalidInput, ""));
            }
            if len != writer.write(&buffer[..len]).await? {
                return Err(BuckyError::new(BuckyErrorCode::InvalidInput, ""));
            }
        }
        
        return Ok(cache)
    }

    fn try_download(&self, stream_cache: ChunkStreamCache) {
        let stack = Stack::from(&self.0.stack);
        let mut sources = self.context().sources_of(|source| {
            if source.object_id.is_none() || source.object_id.as_ref().unwrap() == self.chunk().as_object_id() {
                true
            } else {
                false
            }
        }, 1);

        if sources.len() > 0 {
            let cache = stack.ndn().chunk_manager2().raw_caches().alloc_mem(self.chunk().len());
            stream_cache.load(false, cache.clone_as_raw_cache());

            let source = sources.pop_front().unwrap();
            let channel = stack.ndn().channel_manager().create_channel(&source.target);

            let session = DownloadSession2::new( 
                self.chunk().clone(), 
                stack.ndn().chunk_manager().gen_session_id(), 
                channel, 
                source.referer, 
                ChunkEncodeDesc::Stream(None, None, None), 
                stream_cache.clone()
            );

            let state = &mut *self.0.state.write().unwrap();
            match state {
                StateImpl::Loading => {
                    let downloading = DownloadingState {
                        cache: stream_cache.clone(), 
                        session: Some(session.clone())
                    };
                    *state = StateImpl::Downloading(downloading);
                }, 
                _ => {}
            }
        } 
    }

    pub fn context(&self) -> &MultiDownloadContext {
        &self.0.context
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }


    pub fn state(&self) -> DownloadTaskState {
        self.0.state.read().unwrap().to_task_state()
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
