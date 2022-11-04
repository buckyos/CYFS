
use async_trait::async_trait;
use std::{
    collections::BTreeMap, 
    sync::{Arc, RwLock},
};
use async_std::{
    io::Cursor
};
use cyfs_base::*;
use cyfs_util::*;
use crate::{
    ndn::{ChunkReader}
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

    pub async fn write_chunk<R: async_std::io::Read + Unpin>(&self, id: &ChunkId, reader: R) -> BuckyResult<()> {
        let mut buffer = vec![0u8; id.len()];
        async_std::io::copy(reader, Cursor::new(&mut buffer[..])).await?;
        self.add(id.clone(), Arc::new(buffer)).await
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

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>> {
        let content = self.0.chunks.read().unwrap().get(chunk).cloned()
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "chunk not exists"))?;

        struct ArcWrap(Arc<Vec<u8>>);
        impl AsRef<[u8]> for ArcWrap {
            fn as_ref(&self) -> &[u8] {
                self.0.as_ref()
            }
        }
        
        Ok(Box::new(Cursor::new(ArcWrap(content))))
    }
}







