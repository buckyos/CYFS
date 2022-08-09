use async_std::{
    task, 
    sync::{Arc}
};
use std::{
    sync::{RwLock}
};
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
    storage::ChunkReader, 
    download::*, 
    upload::*
};

use super::super::super::{
    chunk::*, 
};


//TODO: 可能包含其他的内存状态，比如本地不存在但是但是left right有之类
struct StateImpl {
    state: ChunkState, 
    // 在任何状态下都可以由uploader 的容器
    uploader: Option<ChunkUploader>, 
    downloader: Option<ChunkDownloader>, 
    raptor_encoder: Option<RaptorEncoder>,
    raptor_decoder: Option<RaptorDecoder>,
    chunk_cache: Option<Arc<Vec<u8>>>, 
}

struct ViewImpl {
    stack: WeakStack,
    chunk: ChunkId,  
    resource: ResourceManager, 
    state: RwLock<StateImpl>, 
}

struct ViewCacheReader {
    store_reader: Box<dyn ChunkReader>, 
    view: ChunkView
}

impl ViewCacheReader {
    fn new(stack: &Stack, view: ChunkView) -> Arc<Box<dyn ChunkReader>> {
        let reader = Self {
            store_reader: stack.ndn().chunk_manager().store().clone_as_reader(), 
            view
        };
        Arc::new(Box::new(reader))
    }
}

#[async_trait::async_trait]
impl ChunkReader for ViewCacheReader {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(Self {
            store_reader: self.store_reader.clone_as_reader(), 
            view: self.view.clone()
        })
    }

    async fn exists(&self, chunk: &ChunkId) -> bool {
        self.store_reader.exists(chunk).await
    }

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Arc<Vec<u8>>> {
        match self.store_reader.get(chunk).await {
            Ok(content) => {
                let mut state = self.view.0.state.write().unwrap();
                state.state = ChunkState::Ready;
                if state.chunk_cache.is_none() {
                    info!("{} store chunk cache", self.view);
                    state.chunk_cache = Some(content.clone());
                }
                Ok(content)
            }, 
            Err(err) => {
                error!("{} read existing chunk failed for {}", self.view, err);
                let mut state = self.view.0.state.write().unwrap();
                if state.chunk_cache.is_none() {
                    info!("{} reset state to NotFound", self.view);
                    state.state = ChunkState::NotFound;
                }
                Err(err)
            }
        }
        
    }
}

#[derive(Clone)]
pub struct ChunkView(Arc<ViewImpl>);

impl std::fmt::Display for ChunkView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkView:{{local:{}, chunk:{}}}", Stack::from(&self.0.stack).local_device_id(), self.chunk())
    }
}

impl ChunkView {
    pub fn raptor_decoder(&self) -> RaptorDecoder {
        let mut state = self.0.state.write().unwrap();
        match &state.raptor_decoder {
            Some(decoder) => decoder.clone(), 
            None => {
                let decoder = RaptorDecoder::new(self.chunk());
                state.raptor_decoder = Some(decoder.clone());
                decoder
            }
        }
    }

    pub fn raptor_encoder(&self) -> RaptorEncoder {
        let reader = self.reader().unwrap();
        let mut state = self.0.state.write().unwrap();
        match &state.raptor_encoder {
            Some(encoder) => encoder.clone(), 
            None => {
                let encoder = RaptorEncoder::from_reader(reader, self.chunk());
                state.raptor_encoder  = Some(encoder.clone());
                encoder
            }
        }
    }

    pub fn new(
        stack: WeakStack, 
        chunk: ChunkId,  
        init_state: &ChunkState,
    ) -> Self {
            
            Self(Arc::new(ViewImpl {
                stack, 
                chunk, 
                resource: ResourceManager::new(None), 
                state: RwLock::new(StateImpl {
                    state: *init_state, 
                    uploader: None, 
                    downloader: None, 
                    raptor_encoder: None,
                    raptor_decoder: None, 
                    chunk_cache: None,
                }),
            }))
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn recyclable(&self, expect_ref: usize) -> bool {
        Arc::strong_count(&self.0) == expect_ref
    }

    pub fn resource(&self) -> &ResourceManager {
        &self.0.resource
    }

    pub async fn load(&self) -> BuckyResult<()> {
        if self.0.state.read().unwrap().state != ChunkState::Unknown {
            return Ok(());
        }

        let stack = Stack::from(&self.0.stack);
        let view = self.clone();

        if stack.ndn().chunk_manager().store().exists(self.chunk()).await {
            let mut state = view.0.state.write().unwrap();
            if state.state == ChunkState::Unknown {
                state.state = ChunkState::Ready;
            }
        } else {
            let mut state = view.0.state.write().unwrap();
            if state.state == ChunkState::Unknown {
                state.state = ChunkState::NotFound;
            }
        }

        Ok(())
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    pub fn start_download(
        &self, 
        config: Arc<ChunkDownloadConfig>, 
        owner: ResourceManager
    ) -> BuckyResult<ChunkDownloader> {
        let (downloader, newly) = {
            let mut state = self.0.state.write().unwrap();
            match &state.state {
                ChunkState::NotFound => {
                    let newly = if state.downloader.is_none() {
                        info!("{} will create downloader", self);
                        state.downloader = Some(ChunkDownloader::new(
                            self.0.stack.clone(), 
                            self.chunk().clone())
                        );
                        state.state = ChunkState::Pending;
                        true
                    }  else {
                        false
                    };
                    (state.downloader.as_ref().unwrap().clone(), newly)
                }, 
                ChunkState::Ready => {
                    // 直接返回处于finish状态的Downloader
                    let reader = if state.chunk_cache.is_some() {
                        Arc::new(Box::new(CacheReader {
                            cache: state.chunk_cache.as_ref().unwrap().clone()
                        }) as Box<dyn ChunkReader>)
                    } else {
                        ViewCacheReader::new(&Stack::from(&self.0.stack), self.clone())
                    };
                    (ChunkDownloader::finished(
                        self.0.stack.clone(), 
                        self.chunk().clone(), 
                        reader
                    ), false)
                }, 
                ChunkState::Pending => {
                    return Err(BuckyError::new(BuckyErrorCode::Pending, 
                                               format!("{} is pending, please wait ...", self.chunk())));
                }
                _ => unreachable!()
            }
        };

        if newly {
            let _ = downloader.add_config(config, owner);
            let downloader = downloader.clone();
            let view = self.clone();
            task::spawn(async move {
                info!("{} begin wait downloader finish", view);
                match downloader.wait_finish().await {
                    TaskState::Finished => {
                        let chunk_content = downloader.reader().unwrap().get(view.chunk()).await.unwrap();
                        let mut state = view.0.state.write().unwrap();
                        state.state = ChunkState::Ready;
                        if state.chunk_cache.is_none() {
                            state.chunk_cache = Some(chunk_content);
                        }
                        if state.downloader.is_some() 
                            && state.downloader.as_ref().unwrap().ptr_eq(&downloader) {
                            state.downloader = None;
                        } 
                    },
                    TaskState::Canceled(_) => {
                        // do nothing
                    },
                    _ => unimplemented!()
                } 
            });
        }
        Ok(downloader)
    }

    pub fn start_upload(
        &self, 
        session_id: TempSeq, 
        piece_type: PieceSessionType, 
        to: Channel, 
        owner: ResourceManager) -> BuckyResult<UploadSession> {
        let uploader = {
            let mut state = self.0.state.write().unwrap();
            match &state.state {
                ChunkState::NotFound => Err(BuckyError::new(BuckyErrorCode::NotFound, "chunk not found")),  
                ChunkState::Ready => {
                    if state.uploader.is_none() {
                        info!("{} will create uploader", self);
                        state.uploader = Some(ChunkUploader::new(
                            self.clone(), 
                            owner
                        ));
                    }
                    Ok(state.uploader.as_ref().unwrap().clone())
                }, 
                ChunkState::Pending => Err(BuckyError::new(BuckyErrorCode::Pending, "chunk pending.")),
                _ => unreachable!()
            }
        }?;

        let session = UploadSession::new(
            self.chunk().clone(), 
            session_id, 
            piece_type, 
            to, 
            uploader.resource().clone()
        );
        match uploader.add_session(session.clone()) {
            Ok(_) => Ok(session), 
            Err(err) => {
                let _ = uploader.resource().remove_child(session.resource());
                Err(err)
            }
        }
    }

    pub fn reader(&self) -> Option<Arc<Box<dyn ChunkReader>>> {
        let state = self.0.state.read().unwrap();
        match state.state {
            ChunkState::Ready => {
                let reader = if state.chunk_cache.is_some() {
                    Arc::new(Box::new(CacheReader {
                        cache: state.chunk_cache.as_ref().unwrap().clone()
                    }) as Box<dyn ChunkReader>)
                } else {
                    ViewCacheReader::new(&Stack::from(&self.0.stack), self.clone())
                };
                Some(reader)
            },
            _ => None
        }
    }
}

impl Scheduler for ChunkView {
    fn collect_resource_usage(&self) {
        let mut state = self.0.state.write().unwrap();
        if state.state != ChunkState::Unknown {
            if let Some(downloader) = state.downloader.as_ref() {
                let task_state = downloader.schedule_state();
                if match task_state {
                    TaskState::Finished => true, 
                    TaskState::Canceled(_) => true, 
                    _ => false 
                } {
                    info!("{} remove downloader for finished/canceled", self);
                    state.downloader = None;
                }
            } 

            if let Some(uploader) = state.uploader.as_ref() {
                let task_state = uploader.schedule_state();
                if match task_state {
                    TaskState::Finished => true, 
                    TaskState::Canceled(_) => true, 
                    _ => false 
                } {
                    info!("{} remove uploader for finished/canceled", self);
                    state.uploader = None;
                }
            }
        }
    }

    fn schedule_resource(&self) {

    }

    fn apply_scheduled_resource(&self) {

    }
}

