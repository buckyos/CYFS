
use async_trait::async_trait;
use std::{
    collections::BTreeMap, 
    sync::{Arc, RwLock},
};
use cyfs_base::*;
use cyfs_util::cache::*;
use crate::{
    ndn::{ChunkWriter, ChunkReader}
};

struct StoreImpl {
    ndc: Box<dyn NamedDataCache>, 
    chunks: RwLock<BTreeMap<ChunkId, Arc<Vec<u8>>>>
}

#[derive(Clone)]
pub struct MemChunkStore(Arc<StoreImpl>);

impl std::fmt::Display for MemChunkStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemChunkStore")
    }
}

impl MemChunkStore {
    pub fn new(ndc: &dyn NamedDataCache) -> Self {
        Self(Arc::new(StoreImpl {
            ndc: NamedDataCache::clone(ndc), 
            chunks: RwLock::new(BTreeMap::new())
        }))
    }

    pub async fn add(&self, id: ChunkId, chunk: Arc<Vec<u8>>) -> BuckyResult<()> {
        let request = InsertChunkRequest {
            chunk_id: id.to_owned(),
            state: ChunkState::Ready,
            ref_objects: None,
            trans_sessions: None,
            flags: 0,
        };

        let _ = self.0.ndc.insert_chunk(&request).await.map_err(|e| {
            error!("record file chunk to ndc error! chunk={}, {}",id, e);
            e
        });

        self.0.chunks.write().unwrap().insert(id, chunk);

        Ok(())
    }
}


#[async_trait]
impl ChunkReader for MemChunkStore {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(self.clone())
    }

    async fn exists(&self, chunk: &ChunkId) -> bool {
        self.0.chunks.read().unwrap().get(chunk).is_some()
    }

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Arc<Vec<u8>>> {
        self.0.chunks.read().unwrap().get(chunk).cloned()
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "chunk not exists"))
    }
}


#[async_trait]
impl ChunkWriter for MemChunkStore {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone())
    }

    async fn err(&self, _e: BuckyErrorCode) -> BuckyResult<()> {
        Ok(())
    }

    async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        if chunk.len() == 0 {
            return Ok(());
        }

        self.add(chunk.clone(), content).await
    }

    async fn finish(&self) -> BuckyResult<()> {
        // do nothing
        Ok(())
    }
}






