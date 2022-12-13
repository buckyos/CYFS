use std::{
    collections::{BTreeMap, BTreeSet}, 
    sync::RwLock, 
    str::FromStr
};
use async_std::{
    sync::Arc
};
use cyfs_base::*;
use cyfs_util::cache::*;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ChunkStoreStub {
    path: String, 
    range_begin: u64, 
    range_end: u64
}

impl ChunkStoreStub {
    fn to_cache_data(&self) -> TrackerPositionCacheData {
        TrackerPositionCacheData {
            direction: TrackerDirection::Store,
            pos: TrackerPostion::FileRange(PostionFileRange {
                path: self.path.clone(),
                range_begin: self.range_begin,
                range_end: self.range_end,
            }),
            insert_time: 0,
            flags: 0,
        }
    }
}

struct ChunkStub {
    state: ChunkState, 
    owners: BTreeSet<ObjectId>, 
    positions: BTreeSet<ChunkStoreStub>
}

impl ChunkStub {
    fn to_cache_data(&self, chunk: &ChunkId) -> ChunkCacheData {
        ChunkCacheData {
            chunk_id: chunk.clone(), 
            state: self.state, 
            flags: 0, 
            insert_time: 0, 
            update_time: 0, 
            last_access_time: 0, 
            trans_sessions: None, 
            ref_objects: if self.owners.len() > 0 {
                Some(self.owners.iter().map(|o| ChunkObjectRef {
                    object_id: o.clone(), 
                    relation: ChunkObjectRelation::FileBody
                }).collect())
            } else {
                None
            },
        }
    }
}

struct TrackerImpl {
    chunks: BTreeMap<ChunkId, ChunkStub>, 
}

#[derive(Clone)]
pub struct MemTracker(Arc<RwLock<TrackerImpl>>);

impl MemTracker {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(TrackerImpl {
            chunks: BTreeMap::new()
        })))
    }
}

#[async_trait::async_trait]
impl NamedDataCache for MemTracker {
    fn clone(&self) -> Box<dyn NamedDataCache> {
        Box::new(Self(self.0.clone()))
    }

    // file相关接口
    async fn insert_file(&self, _req: &InsertFileRequest) -> BuckyResult<()> {
        Ok(())
    }

    async fn remove_file(&self, _req: &RemoveFileRequest) -> BuckyResult<usize> {
        Ok(0)
    }

    async fn file_update_quick_hash(&self, _req: &FileUpdateQuickhashRequest) -> BuckyResult<()> {
        Ok(())
    }

    async fn get_file_by_hash(&self, _req: &GetFileByHashRequest) -> BuckyResult<Option<FileCacheData>> {
        Ok(None)
    }

    async fn get_file_by_file_id(&self, _req: &GetFileByFileIdRequest) -> BuckyResult<Option<FileCacheData>> {
        Ok(None)
    }

    async fn get_files_by_quick_hash(&self, _req: &GetFileByQuickHashRequest) -> BuckyResult<Vec<FileCacheData>> {
        Ok(vec![])
    }

    async fn get_files_by_chunk(&self, _req: &GetFileByChunkRequest) -> BuckyResult<Vec<FileCacheData>> {
        Ok(vec![])
    }

    async fn get_dirs_by_file(&self, _req: &GetDirByFileRequest) -> BuckyResult<Vec<FileDirRef>> {
        Ok(vec![])
    }


    async fn insert_chunk(&self, req: &InsertChunkRequest) -> BuckyResult<()> {
        let mut tracker = self.0.write().unwrap();
        tracker.chunks.entry(req.chunk_id.clone()).or_insert(ChunkStub {
            state: req.state, 
            owners: {
                let mut owners = BTreeSet::new();
                if let Some(ref_objects) = req.ref_objects.as_ref() {
                    for r in ref_objects {
                        owners.insert(r.object_id.clone());
                    }
                } 
                owners
            }, 
            positions: BTreeSet::new()
        });
        Ok(())
    }

    async fn remove_chunk(&self, req: &RemoveChunkRequest) -> BuckyResult<usize> {
        let mut tracker = self.0.write().unwrap();
        Ok(if tracker.chunks.remove(&req.chunk_id).is_some() {
            1
        } else {
            0
        })
    }

    async fn update_chunk_state(&self, req: &UpdateChunkStateRequest) -> BuckyResult<ChunkState> {
        let mut tracker = self.0.write().unwrap();
        if let Some(stub) = tracker.chunks.get_mut(&req.chunk_id) {
            stub.state = req.state;
            Ok(req.state)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "chunk not cached"))
        }
    }
    
    async fn update_chunk_ref_objects(&self, req: &UpdateChunkRefsRequest) -> BuckyResult<()> {
        let mut tracker = self.0.write().unwrap();
        if let Some(stub) = tracker.chunks.get_mut(&req.chunk_id) {
            for a in &req.add_list {
                stub.owners.insert(a.object_id.clone());
            }
            for r in &req.remove_list {
                stub.owners.remove(&r.object_id);
            }
            Ok(())
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "chunk not cached"))
        }
    }

    async fn exists_chunks(&self, req: &ExistsChunkRequest) -> BuckyResult<Vec<bool>> {
        let inner = self.0.read().unwrap();
        Ok(req.chunk_list.iter().map(|chunk_id| {
            if let Some(info) = inner.chunks.get(chunk_id) {
                req.states.iter().find(|&state| *state == info.state).is_some()
            } else {
                false
            }
        }).collect())
    }

    async fn get_chunk(&self, req: &GetChunkRequest) -> BuckyResult<Option<ChunkCacheData>> {
        Ok(self.0.read().unwrap().chunks.get(&req.chunk_id).map(|stub| stub.to_cache_data(&req.chunk_id)))
    }

    async fn get_chunks(&self, req: &Vec<GetChunkRequest>) -> BuckyResult<Vec<Option<ChunkCacheData>>> {
        let tracker = self.0.read().unwrap();
        Ok(req.iter().map(|r| tracker.chunks.get(&r.chunk_id).map(|stub| stub.to_cache_data(&r.chunk_id))).collect())
    }

    async fn get_chunk_ref_objects(&self, req: &GetChunkRefObjectsRequest) -> BuckyResult<Vec<ChunkObjectRef>> {
        self.0.read().unwrap().chunks.get(&req.chunk_id)
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "chunk not cached"))
            .map(|stub| stub.owners.iter().map(|o| ChunkObjectRef {
                object_id: o.clone(), 
                relation: ChunkObjectRelation::FileBody
            }).collect())
    }
}

#[async_trait::async_trait]
impl TrackerCache for MemTracker {
    fn clone(&self) -> Box<dyn TrackerCache> {
        Box::new(Self(self.0.clone()))
    }

    async fn add_position(&self, req: &AddTrackerPositonRequest) -> BuckyResult<()> {
        let chunk = ChunkId::from_str(req.id.as_str())?;
        let mut tracker = self.0.write().unwrap();
        tracker.chunks.get_mut(&chunk)
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "chunk not cached"))
            .map(|stub| {
                match &req.pos {
                    TrackerPostion::File(p) => {
                        let _ = stub.positions.insert(ChunkStoreStub {
                            path: p.clone(), 
                            range_begin: 0, 
                            range_end: chunk.len() as u64
                        });
                    },
                    TrackerPostion::FileRange(p) => {
                        let _ = stub.positions.insert(ChunkStoreStub {
                            path: p.path.clone(), 
                            range_begin: p.range_begin, 
                            range_end: p.range_end
                        });
                    },
                    _ => {}
                }
                ()
            })
    }

    async fn remove_position(&self, req: &RemoveTrackerPositionRequest) -> BuckyResult<usize> {
        let chunk = ChunkId::from_str(req.id.as_str())?;
        let mut tracker = self.0.write().unwrap();
        tracker.chunks.get_mut(&chunk)
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "chunk not cached"))
            .map(|stub| if let Some(p) = &req.pos {
                match p {
                    TrackerPostion::FileRange(p) => {
                        if stub.positions.remove(&ChunkStoreStub {
                            path: p.path.clone(), 
                            range_begin: p.range_begin, 
                            range_end: p.range_end
                        }) {
                            1
                        } else {
                            0
                        }
                    },
                    _ => 0
                }
                
            } else {
                let mut empty = BTreeSet::new();
                std::mem::swap(&mut stub.positions, &mut empty);
                empty.len()
            })
    }

    async fn get_position(
        &self,
        req: &GetTrackerPositionRequest,
    ) -> BuckyResult<Vec<TrackerPositionCacheData>> {
        let chunk = ChunkId::from_str(req.id.as_str())?;
        self.0.read().unwrap().chunks.get(&chunk)
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "chunk not cached"))
            .map(|stub| stub.positions.iter().map(|p| p.to_cache_data()).collect())
    }
} 