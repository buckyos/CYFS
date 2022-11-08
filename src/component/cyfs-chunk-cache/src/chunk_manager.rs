use std::path::PathBuf;
use std::sync::{Arc, Once, RwLock};
use cyfs_chunk_lib::{Chunk, ChunkMeta, ChunkMut};
use cyfs_base::*;
use crate::{ChunkCache, LocalChunkCache, SingleDiskChunkCache, DiskScanner, ChunkType};

static mut CHUNK_MANAGER_INSTANCE: Option<ChunkManager> = None;
static CHUNK_MANAGER_INIT: Once = Once::new();

pub struct CYFSDiskScanner;

impl DiskScanner for CYFSDiskScanner {
    fn get_cache_path_list(&self) -> Vec<(PathBuf, u64)> {
        vec![(cyfs_util::get_cyfs_root_path().join("data").join("chunk-cache"), 1024*1024*1024*1024)]
    }
}

pub struct ChunkManager {
    chunk_cache: RwLock<Option<Arc<dyn ChunkCache>>>
}

pub type ChunkManagerRef = Arc<ChunkManager>;

impl ChunkManager {
    pub fn new() -> Self {
        Self {
            chunk_cache: RwLock::new(None)
        }
    }

    pub async fn init(&self, isolate: &str) -> BuckyResult<()> {
        let chunk_cache: Arc<dyn ChunkCache> = Arc::new(LocalChunkCache::<SingleDiskChunkCache, CYFSDiskScanner>::new(isolate, CYFSDiskScanner).await?);
        *self.chunk_cache.write().unwrap() = Some(chunk_cache);
        Ok(())
    }

    pub async fn get_chunk(&self, chunk_id: &ChunkId, chunk_type: ChunkType) -> BuckyResult<Box<dyn Chunk>> {
        let chunk_cache = {
            let chunk_cache = self.chunk_cache.read().unwrap();
            chunk_cache.as_ref().unwrap().clone()
        };
        chunk_cache.get_chunk(chunk_id, chunk_type).await
    }

    pub async fn new_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<Box<dyn ChunkMut>> {
        let chunk_cache = {
            let chunk_cache = self.chunk_cache.read().unwrap();
            chunk_cache.as_ref().unwrap().clone()
        };
        chunk_cache.new_chunk(chunk_id).await
    }

    pub async fn delete_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<()> {
        let chunk_cache = {
            let chunk_cache = self.chunk_cache.read().unwrap();
            chunk_cache.as_ref().unwrap().clone()
        };
        chunk_cache.delete_chunk(chunk_id).await
    }

    pub async fn put_chunk(&self, chunk_id: &ChunkId, chunk: &dyn Chunk) -> BuckyResult<()> {
        let chunk_cache = {
            let chunk_cache = self.chunk_cache.read().unwrap();
            chunk_cache.as_ref().unwrap().clone()
        };
        chunk_cache.put_chunk(chunk_id, chunk).await
    }

    pub async fn exist(&self, chunk_id: &ChunkId) -> bool {
        let chunk_cache = {
            let chunk_cache = self.chunk_cache.read().unwrap();
            chunk_cache.as_ref().unwrap().clone()
        };
        chunk_cache.is_exist(chunk_id).await
    }

    pub async fn get_chunk_meta(&self, chunk_id: &ChunkId, chunk_type: ChunkType) -> BuckyResult<ChunkMeta> {
        let chunk_cache = {
            let chunk_cache = self.chunk_cache.read().unwrap();
            chunk_cache.as_ref().unwrap().clone()
        };
        chunk_cache.get_chunk_meta(chunk_id, chunk_type).await
    }
}

#[cfg(test)]
mod test_chunk_mananger {
    #[test]
    fn test() {
        // async_std::task::block_on(async {
        //     ChunkManager::init().unwrap();
        //     for i in 0..10000 {
        //         let mut buf: Vec<u8> = Vec::new();
        //         buf.resize(4096, i as u8);
        //         let rand: u32 = rand::random();
        //         unsafe {
        //             std::ptr::copy(rand.to_be_bytes().as_ptr(), buf.as_mut_ptr(), 4);
        //         }
        //
        //         let chunk: Box<dyn Chunk> = Box::new(MemChunk::from(buf));
        //         ChunkManager::current().put_chunk(chunk.calculate_id(), &chunk).await.unwrap();
        //     }
        // })
    }
}
