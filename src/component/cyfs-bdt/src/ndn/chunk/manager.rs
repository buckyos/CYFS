use std::{
    collections::{BTreeMap}, 
    sync::{RwLock},
};
use async_std::{
    io::Cursor
};
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_util::*;
use crate::{
    types::*, 
    stack::{WeakStack, Stack},
};
use super::{
    storage::*,  
    cache::*
};

pub struct ChunkManager {
    stack: WeakStack, 
    ndc: Box<dyn NamedDataCache>, 
    tracker: Box<dyn TrackerCache>, 
    store: Box<dyn ChunkReader>, 
    gen_session_id: TempSeqGenerator, 
    raw_caches: RawCacheManager, 
    chunk_caches: RwLock<BTreeMap<ChunkId, ChunkCache>>, 
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

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>> {
        if chunk.len() == 0 {
            Ok(Box::new(Cursor::new(vec![0u8; 0])))
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
            raw_caches: RawCacheManager::new(), 
            chunk_caches: RwLock::new(Default::default())
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

    pub fn raw_caches(&self) -> &RawCacheManager {
        &self.raw_caches
    }

    pub(super) fn gen_session_id(&self) -> TempSeq {
        self.gen_session_id.generate()
    }

    pub fn create_cache(&self, chunk: &ChunkId) -> ChunkCache {
        let mut caches = self.chunk_caches.write().unwrap();
        if let Some(cache) = caches.get(chunk).cloned() {
            cache
        } else {
            let cache = ChunkCache::new(self.stack.clone(), chunk.clone());
            caches.insert(chunk.clone(), cache.clone());
            cache
        }
    }

    pub fn cache_of(&self, chunk: &ChunkId) -> Option<ChunkCache> {
        self.chunk_caches.read().unwrap().get(chunk).cloned()
    }

}