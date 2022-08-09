use std::{
    collections::{BTreeMap, LinkedList}, 
    sync::{RwLock},
};
use async_std::{
    sync::Arc, 
};
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_util::cache::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack}
};
use super::super::{
    scheduler::*, 
    channel::{PieceSessionType, Channel, UploadSession}
};
use super::{
    storage::*,  
    download::{ChunkDownloader, ChunkDownloadConfig}, 
    view::ChunkView
};



pub struct ChunkManager {
    stack: WeakStack, 
    ndc: Box<dyn NamedDataCache>, 
    tracker: Box<dyn TrackerCache>, 
    store: Box<dyn ChunkReader>, 
    gen_session_id: TempSeqGenerator, 
    views: RwLock<BTreeMap<ChunkId, ChunkView>>, 
}

impl std::fmt::Display for ChunkManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkManager:{{local:{}}}", Stack::from(&self.stack).local_device_id())
    }
}


struct EmptyChunkWrapper(Box<dyn ChunkReader>);

impl EmptyChunkWrapper {
    fn new(non_empty: Box<dyn ChunkReader>) -> Self {
        Self(non_empty)
    }
}

#[async_trait]
impl ChunkReader for EmptyChunkWrapper {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(Self(self.0.clone_as_reader()))
    }

    async fn exists(&self, chunk: &ChunkId) -> bool {
        if chunk.len() == 0 {
            true
        } else {
            self.0.exists(chunk).await
        }
    }

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Arc<Vec<u8>>> {
        if chunk.len() == 0 {
            Ok(Arc::new(vec![0u8; 0]))
        } else {
            self.0.get(chunk).await
        }
    }
}

impl ChunkManager {
    pub(crate) fn new(
        weak_stack: WeakStack, 
        ndc: Box<dyn NamedDataCache>, 
        tracker: Box<dyn TrackerCache>, 
        store: Box<dyn ChunkReader>
    ) -> Self {
        Self { 
            stack: weak_stack, 
            gen_session_id: TempSeqGenerator::new(), 
            ndc, 
            tracker, 
            store: Box::new(EmptyChunkWrapper::new(store)), 
            views: RwLock::new(BTreeMap::new())
        }
    }

    pub async fn track_chunk(&self, chunk: &ChunkId) -> BuckyResult<()> {
        let request = InsertChunkRequest {
            chunk_id: chunk.to_owned(),
            state: ChunkState::Unknown,
            ref_objects: None,
            trans_sessions: None,
            flags: 0,
        };

        self.ndc().insert_chunk(&request).await.map_err(|e| {
            error!("record file chunk to ndc error! chunk={}, {}",chunk, e);
            e
        })
    }

    pub async fn track_file(&self, file: &File) -> BuckyResult<()> {
        let file_id = file.desc().calculate_id();
        match file.body() {
            Some(body) => {
                let chunk_list = body.content().inner_chunk_list();
                match chunk_list {
                    Some(chunks) => {
                        for chunk in chunks {
                            // 先添加到chunk索引
                            let ref_obj = ChunkObjectRef {
                                object_id: file_id.to_owned(),
                                relation: ChunkObjectRelation::FileBody,
                            };
                
                            let req = InsertChunkRequest {
                                chunk_id: chunk.to_owned(),
                                state: ChunkState::Unknown,
                                ref_objects: Some(vec![ref_obj]),
                                trans_sessions: None,
                                flags: 0,
                            };
                
                            self.ndc().insert_chunk(&req).await.map_err(|e| {
                                error!("record file chunk to ndc error! file={}, chunk={}, {}", file_id, chunk, e);
                                e
                            })?;

                            info!("insert chunk of file to ndc, chunk:{}, file:{}", chunk, file_id);
                        }
                        Ok(())
                    }
                    None => Err(BuckyError::new(
                        BuckyErrorCode::NotSupport,
                        format!("file object should has chunk list: {}", file_id),
                    )),
                }
            }
            None => {
                Err(BuckyError::new(
                    BuckyErrorCode::InvalidFormat,
                    format!("file object should has body: {}", file_id),
                ))
            }
        }

        
    }

    pub fn ndc(&self) -> &dyn NamedDataCache {
        self.ndc.as_ref()
    }

    pub fn tracker(&self) -> &dyn TrackerCache {
        self.tracker.as_ref()
    }

    pub fn store(&self) -> &dyn ChunkReader {
        self.store.as_ref()
    }
}


impl ChunkManager {
    pub fn view_of(&self, chunk: &ChunkId) -> Option<ChunkView> {
        self.views.read().unwrap().get(chunk).cloned()
    }

    async fn create_view(&self, chunk: ChunkId, init_state: ChunkState) -> BuckyResult<ChunkView> {
        let view = self.views.read().unwrap().get(&chunk).cloned();
        let view = match view {
            Some(view) => view, 
            None => {
                info!("{} will create chunk view of {}", self, chunk);
                let view = ChunkView::new(
                    self.stack.clone(), 
                    chunk.clone(), 
                    &init_state);
                let mut views = self.views.write().unwrap();
                match views.get(&chunk) {
                    Some(view) => view.clone(), 
                    None => {
                        views.insert(chunk.clone(), view.clone());
                        view
                    }
                } 
            }
        };
        view.load().await?;
        Ok(view)
    }

    pub(crate) async fn start_download(
        &self, 
        chunk: ChunkId, 
        config: Arc<ChunkDownloadConfig>, 
        owner: ResourceManager
    ) -> BuckyResult<ChunkDownloader> {
        info!("{} try start download config: {:?}", self, &*config);
        let view = self.create_view(chunk, ChunkState::Unknown).await?;
        view.start_download(config, owner)
    }

    pub(crate) async fn start_upload(
        &self, 
        session_id: TempSeq, 
        chunk: ChunkId, 
        piece_type: PieceSessionType, 
        to: Channel, 
        owner: ResourceManager
    ) -> BuckyResult<UploadSession> {
        info!("{} try start upload type: {:?} to: {}", self, piece_type, to.remote());
        let view = self.create_view(chunk, ChunkState::Unknown).await?;
        view.start_upload(session_id, piece_type, to, owner)
            .map_err(|err| {
                error!("{} failed start upload for {}", self, err);
                err
            })
    }


    pub(super) fn gen_session_id(&self) -> TempSeq {
        self.gen_session_id.generate()
    }
}

impl Scheduler for ChunkManager {
    fn collect_resource_usage(&self) {
        let views: Vec<ChunkView> = self.views.read().unwrap().values().cloned().collect();
        let mut to_recycle = LinkedList::new();
        for view in views {
            view.collect_resource_usage();
            if view.recyclable(2) {
                to_recycle.push_back(view);
            }
        }

        if to_recycle.len() > 0 {
            let mut views = self.views.write().unwrap();
            for view in to_recycle {
                if let Some(exists) = views.remove(view.chunk()) {
                    if view.ptr_eq(&exists) && view.recyclable(2) {
                        info!("{} recycle {}", self, view);
                    } else {
                        views.insert(view.chunk().clone(), exists);
                    }
                }
            }
        }
    }

    fn schedule_resource(&self) {
        //TODO
    }

    fn apply_scheduled_resource(&self) {
        //TODO
    }
}