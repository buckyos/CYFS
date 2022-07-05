use super::assoc::AssociationObjects;
use super::cache::SyncObjectsStateCache;
use crate::ndn_api::ChunkStoreReader;
use cyfs_base::*;
use cyfs_bdt::{ChunkReader, StackGuard};
use cyfs_chunk_cache::ChunkManager;
use cyfs_lib::*;

use async_std::io::prelude::*;
use std::borrow::Cow;
use std::sync::Arc;

/*
1. 需要优先同步body.chunk，如果同步失败，由于无法确定desc里面的chunk是否在body里面，会停止同步
2. 同步desc.chunk，如果desc.chunk不再body里面，那么需要再次同步
3. 同步desc.objlist
*/

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirSyncState {
    Init,
    BodyChunkPending,
    BodyChunkComplete,
    DescChunkPending,
    DescChunkComplete,
    Complete,
}

struct DirSync {
    chunk_reader: Arc<ChunkStoreReader>,
    dir_id: ObjectId,
    dir: Dir,
    desc_obj_list: Option<NDNObjectList>,
    body_obj_list: Option<DirBodyContentObjectList>,
    state: DirSyncState,
    state_cache: SyncObjectsStateCache,
}

impl DirSync {
    pub fn new(
        chunk_reader: Arc<ChunkStoreReader>,
        state_cache: SyncObjectsStateCache,
        dir_id: ObjectId,
        dir: Dir,
    ) -> Self {
        Self {
            chunk_reader,
            dir_id,
            dir,
            desc_obj_list: None,
            body_obj_list: None,
            state: DirSyncState::Init,
            state_cache,
        }
    }

    pub async fn sync(
        &mut self,
        assoc_objects: &mut AssociationObjects,
    ) -> BuckyResult<DirSyncState> {
        loop {
            let new_state = match self.state {
                DirSyncState::Init | DirSyncState::BodyChunkPending => {
                    self.sync_body_chunk(assoc_objects).await?
                }
                DirSyncState::BodyChunkComplete | DirSyncState::DescChunkPending => {
                    self.sync_desc_chunk(assoc_objects).await?
                }
                DirSyncState::DescChunkComplete => {
                    self.sync_desc_obj_list(assoc_objects).await?
                }
                DirSyncState::Complete => {
                    break;
                }
            };

            assert!(new_state != self.state);
            self.state = new_state;
            
            if self.state == DirSyncState::BodyChunkPending
                || self.state == DirSyncState::DescChunkPending
            {
                break;
            }
        }

        Ok(self.state)
    }

    async fn sync_body_chunk(
        &mut self,
        assoc_objects: &mut AssociationObjects,
    ) -> BuckyResult<DirSyncState> {
        assert!(self.state == DirSyncState::Init || self.state == DirSyncState::BodyChunkPending);

        if let Some(body) = self.dir.body() {
            match body.content() {
                DirBodyContent::Chunk(id) => {
                    if self.state_cache.is_object_missing(id.as_object_id()) {
                        return Ok(DirSyncState::Complete);
                    }

                    let ret = self.chunk_reader.read(id).await;
                    match ret {
                        Ok(mut reader) => {
                            let mut buf = vec![];
                            reader.read_to_end(&mut buf).await?;

                            let obj_list = DirBodyContentObjectList::clone_from_slice(&buf)?;
                            assert!(self.body_obj_list.is_none());
                            self.body_obj_list = Some(obj_list);

                            Ok(DirSyncState::BodyChunkComplete)
                        }
                        Err(e) => match e.code() {
                            BuckyErrorCode::NotFound => match self.state {
                                DirSyncState::Init => {
                                    debug!("dir body chunk not exists, now will sync: dir={}, chunk={}", self.dir_id, id);
                                    assoc_objects.append_item(id.as_object_id());
                                    Ok(DirSyncState::BodyChunkPending)
                                }
                                DirSyncState::BodyChunkPending => Ok(DirSyncState::Complete),
                                _ => unreachable!(),
                            },
                            _ => {
                                error!("load body chunk from reader failed! chunk={}, {}", id, e);
                                Err(e)
                            }
                        },
                    }
                }
                DirBodyContent::ObjList(_list) => Ok(DirSyncState::BodyChunkComplete),
            }
        } else {
            Ok(DirSyncState::BodyChunkComplete)
        }
    }

    async fn sync_desc_chunk(
        &mut self,
        assoc_objects: &mut AssociationObjects,
    ) -> BuckyResult<DirSyncState> {
        assert!(
            self.state == DirSyncState::DescChunkPending
                || self.state == DirSyncState::BodyChunkComplete
        );

        match self.dir.desc().content().obj_list() {
            NDNObjectInfo::Chunk(chunk_id) => {
                match self.load_chunk_from_body_and_reader(chunk_id).await? {
                    Some(chunk) => {
                        let obj_list = NDNObjectList::clone_from_slice(&chunk)?;
                        assert!(self.desc_obj_list.is_none());
                        self.desc_obj_list = Some(obj_list);

                        Ok(DirSyncState::DescChunkComplete)
                    }
                    None => match self.state {
                        DirSyncState::BodyChunkComplete => {
                            debug!("dir desc chunk not exists, now will sync: dir={}, chunk={}", self.dir_id, chunk_id);
                            assoc_objects.append_item(chunk_id.as_object_id());
                            Ok(DirSyncState::DescChunkPending)
                        }
                        DirSyncState::DescChunkPending => Ok(DirSyncState::Complete),
                        _ => unreachable!(),
                    },
                }
            }
            NDNObjectInfo::ObjList(_list) => Ok(DirSyncState::DescChunkComplete),
        }
    }

    async fn load_chunk_from_body_and_reader<'a>(
        &'a self,
        chunk_id: &ChunkId,
    ) -> BuckyResult<Option<Cow<'a, Vec<u8>>>> {
        let chunk = if let Some(body) = self.dir.body() {
            match body.content() {
                DirBodyContent::Chunk(_) => {
                    if let Some(list) = &self.body_obj_list {
                        list.get(chunk_id.as_object_id())
                    } else {
                        None
                    }
                }
                DirBodyContent::ObjList(list) => list.get(chunk_id.as_object_id()),
            }
        } else {
            None
        };

        if chunk.is_some() {
            return Ok(chunk.map(|chunk| Cow::Borrowed(chunk)));
        }

        // load from reader
        let ret = self.chunk_reader.read(chunk_id).await;
        match ret {
            Ok(mut reader) => {
                let mut buf = vec![];
                reader.read_to_end(&mut buf).await?;

                Ok(Some(Cow::Owned(buf)))
            }
            Err(e) => match e.code() {
                BuckyErrorCode::NotFound => Ok(None),
                _ => {
                    error!(
                        "load body chunk from reader failed! chunk={}, {}",
                        chunk_id, e
                    );
                    Err(e)
                }
            },
        }
    }

    async fn sync_desc_obj_list(&mut self, assoc_objects: &mut AssociationObjects) -> BuckyResult<DirSyncState> {
        assert!(self.state == DirSyncState::DescChunkComplete);

        let obj_list = match self.dir.desc().content().obj_list() {
            NDNObjectInfo::Chunk(_chunk_id) => {
                assert!(self.desc_obj_list.is_some());
                self.desc_obj_list.as_ref().unwrap()
            }
            NDNObjectInfo::ObjList(list) => list,
        };

        self.append_desc_obj_list(obj_list, assoc_objects);

        Ok(DirSyncState::Complete)
    }

    fn append_desc_obj_list(&self, list: &NDNObjectList, assoc_objects: &mut AssociationObjects) {
        let body_map = self.body_map();

        if let Some(ref parent) = list.parent_chunk {
            self.append_object(parent.as_object_id(), &body_map, assoc_objects);
        }

        for (_k, v) in &list.object_map {
            match v.node() {
                InnerNode::ObjId(id) => {
                    self.append_object(id, &body_map, assoc_objects);
                }
                InnerNode::Chunk(id) => {
                    self.append_object(id.as_object_id(), &body_map, assoc_objects);
                }
                InnerNode::IndexInParentChunk(_, _) => {}
            }
        }
    }

    fn append_object(
        &self,
        id: &ObjectId,
        body_map: &Option<&DirBodyContentObjectList>,
        assoc_objects: &mut AssociationObjects,
    ) {
        if let Some(body_map) = body_map {
            if body_map.contains_key(id) {
                return;
            }
        }

        if self.state_cache.is_object_missing(id) {
            return;
        }

        debug!("dir assoc object not exists in body and local, now will sync: dir={}, object={}", self.dir_id, id);
        assoc_objects.append_item(id);
    }

    fn body_map(&self) -> Option<&DirBodyContentObjectList> {
        if let Some(body) = self.dir.body() {
            match body.content() {
                DirBodyContent::Chunk(_) => {
                    assert!(self.body_obj_list.is_some());
                    self.body_obj_list.as_ref()
                }
                DirBodyContent::ObjList(list) => Some(list),
            }
        } else {
            None
        }
    }
}

pub(super) struct DirListSync {
    chunk_reader: Arc<ChunkStoreReader>,

    state_cache: SyncObjectsStateCache,

    list: std::collections::HashMap<ObjectId, DirSync>,
}

impl DirListSync {
    pub(super) fn new(
        state_cache: SyncObjectsStateCache,
        bdt_stack: StackGuard,
        chunk_manager: Arc<ChunkManager>,
    ) -> Self {
        let chunk_reader = ChunkStoreReader::new(
            chunk_manager.clone(),
            bdt_stack.ndn().chunk_manager().ndc().clone(),
            bdt_stack.ndn().chunk_manager().tracker().clone(),
        );

        Self {
            chunk_reader: Arc::new(chunk_reader),
            state_cache,
            list: std::collections::HashMap::new(),
        }
    }

    pub fn fork(&self) -> Self {
        Self {
            chunk_reader: self.chunk_reader.clone(),
            state_cache: self.state_cache.clone(),
            list: std::collections::HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn append_dir(&mut self, info: &NONObjectInfo) {
        let dir = info.object.as_ref().unwrap().as_dir();

        let item = DirSync::new(
            self.chunk_reader.clone(),
            self.state_cache.clone(),
            info.object_id.clone(),
            dir.to_owned(),
        );

        self.list.insert(info.object_id.clone(), item);
    }

    pub async fn sync_once(&mut self, assoc_objects: &mut AssociationObjects) {
        let mut removed = vec![];
        for (dir_id, item) in &mut self.list {
            debug!("begin sync dir={}, state={:?}", dir_id, item.state);
            match item.sync(assoc_objects).await {
                Ok(state) => {
                    debug!("end sync dir={}, state={:?}", dir_id, item.state);
                    if state == DirSyncState::Complete {
                        removed.push(dir_id.to_owned());
                    }
                }
                Err(e) => {
                    error!("sync dir object but failed! id={}, {}", dir_id, e);
                    removed.push(dir_id.to_owned());
                }
            }
        }

        for id in removed {
            self.list.remove(&id);
        }
    }
}
