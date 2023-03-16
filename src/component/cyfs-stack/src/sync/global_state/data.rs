use super::super::client::SyncClientRequestor;
use super::super::protocol::SyncChunksRequest;
use super::cache::SyncObjectsStateCache;
use super::dir_sync::DirListSync;
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_bdt::*;
use cyfs_bdt_ext::ChunkListReaderAdapter;
use cyfs_lib::*;

use cyfs_debug::Mutex;
use std::collections::HashSet;
use std::sync::Arc;

struct AssociationChunks {
    list: HashSet<ChunkId>,
}

impl AssociationChunks {
    pub fn new() -> Self {
        Self {
            list: HashSet::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn append(&mut self, object_id: &ObjectId, object: &AnyNamedObject) {
        match object_id.obj_type_code() {
            ObjectTypeCode::File => {
                self.append_file(object.as_file());
            }
            ObjectTypeCode::Dir => {
                self.append_dir(object.as_dir());
            }
            _ => unimplemented!(),
        }
    }

    fn append_file(&mut self, file: &File) {
        match file.body() {
            Some(body) => match body.content().inner_chunk_list() {
                Some(list) => list.iter().for_each(|chunk_id| {
                    self.append_chunk(chunk_id);
                }),
                None => {}
            },
            None => {}
        }
    }

    fn append_dir(&mut self, dir: &Dir) {
        let mut desc_chunks = HashSet::new();
        let content = dir.desc().content().obj_list();
        match content {
            NDNObjectInfo::Chunk(chunk_id) => {
                desc_chunks.insert(chunk_id.to_owned());
            }
            NDNObjectInfo::ObjList(list) => {
                if let Some(chunk_id) = &list.parent_chunk {
                    desc_chunks.insert(chunk_id.to_owned());
                }

                for (_k, v) in &list.object_map {
                    match v.node() {
                        InnerNode::Chunk(chunk_id) => {
                            desc_chunks.insert(chunk_id.to_owned());
                        }
                        _ => {}
                    }
                }
            }
        }

        match dir.body() {
            Some(body) => {
                match body.content() {
                    DirBodyContent::Chunk(chunk_id) => {
                        desc_chunks.insert(chunk_id.to_owned());
                    }
                    DirBodyContent::ObjList(list) => {
                        // remove chunks which already in body content
                        desc_chunks.retain(|chunk_id| !list.contains_key(chunk_id.as_object_id()));
                    }
                }
            }
            None => {}
        }

        desc_chunks.into_iter().for_each(|chunk_id| {
            self.list.insert(chunk_id);
        })
    }

    fn append_chunk(&mut self, chunk_id: &ChunkId) {
        self.list.insert(chunk_id.to_owned());
    }

    pub fn detach_chunks(&mut self) -> Vec<ChunkId> {
        if self.list.is_empty() {
            return vec![];
        }

        let mut result = HashSet::new();
        std::mem::swap(&mut self.list, &mut result);

        let mut list: Vec<ChunkId> = result.into_iter().collect();
        list.sort();
        list
    }
}

#[derive(Clone)]
pub(super) struct ChunksCollector {
    noc: NamedObjectCacheRef,
    device_id: DeviceId,
    chunks: Arc<Mutex<AssociationChunks>>,
}

impl ChunksCollector {
    pub fn new(noc: NamedObjectCacheRef, device_id: DeviceId) -> Self {
        Self {
            noc,
            device_id,
            chunks: Arc::new(Mutex::new(AssociationChunks::new())),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.lock().unwrap().is_empty()
    }

    pub fn detach_chunks(&self) -> Vec<ChunkId> {
        self.chunks.lock().unwrap().detach_chunks()
    }

    pub async fn append(&self, object_id: &ObjectId) -> BuckyResult<()> {
        match object_id.obj_type_code() {
            ObjectTypeCode::File | ObjectTypeCode::Dir => {
                self.append_impl(object_id).await?;
            }
            ObjectTypeCode::Chunk => {
                self.chunks
                    .lock()
                    .unwrap()
                    .append_chunk(object_id.as_chunk_id());
            }
            _ => {}
        }

        Ok(())
    }

    pub fn append_chunk(&self, chunk_id: &ChunkId) {
        // debug!("add assoc chunk: {}", chunk_id);
        self.chunks.lock().unwrap().append_chunk(chunk_id);
    }

    async fn append_impl(&self, object_id: &ObjectId) -> BuckyResult<()> {
        let req = NamedObjectCacheGetObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object_id: object_id.to_owned(),
            last_access_rpath: None,
            flags: 0,
        };

        let resp = self.noc.get_object(&req).await?;
        match resp {
            Some(data) => {
                let object = data.object.object.unwrap();
                self.append_object(object_id, &object);
            }
            None => {
                debug!("file or dir not found in noc: id={}", object_id,);
            }
        }

        Ok(())
    }

    pub fn append_object(&self, object_id: &ObjectId, object: &AnyNamedObject) {
        self.chunks.lock().unwrap().append(object_id, object);
    }
}

pub(super) struct DataSync {
    bdt_stack: StackGuard,
    named_data_components: NamedDataComponents,
    ood_device_id: DeviceId,
    requestor: Arc<SyncClientRequestor>,
    state_cache: SyncObjectsStateCache,
}

impl DataSync {
    pub fn new(
        bdt_stack: StackGuard,
        named_data_components: NamedDataComponents,
        requestor: Arc<SyncClientRequestor>,
        state_cache: SyncObjectsStateCache,
    ) -> Self {
        let ood_device_id = requestor.requestor().remote_device().unwrap();

        Self {
            bdt_stack,
            named_data_components,
            ood_device_id,
            requestor,
            state_cache,
        }
    }

    pub fn create_dir_sync(&self) -> DirListSync {
        DirListSync::new(self.state_cache.clone(), &self.named_data_components)
    }

    async fn filter_exists_chunks(&self, chunk_list: Vec<ChunkId>) -> BuckyResult<Vec<ChunkId>> {
        let ndc = &self.named_data_components.ndc;
        let mut sync_list = vec![];

        for chunks in chunk_list.chunks(16) {
            let list = chunks.to_owned();
            if list.len() == 0 {
                break;
            }

            let req = ExistsChunkRequest {
                chunk_list: list,
                states: vec![ChunkState::Ready],
            };

            let ret = ndc.exists_chunks(&req).await?;

            req.chunk_list
                .into_iter()
                .zip(ret.into_iter())
                .for_each(|(chunk_id, exists)| {
                    if !exists {
                        sync_list.push(chunk_id);
                    }
                });
        }

        Ok(sync_list)
    }

    fn create_file(chunk_list: Vec<ChunkId>) -> File {
        assert!(chunk_list.len() > 0);

        let bundle = ChunkBundle::new(chunk_list, ChunkBundleHashMethod::Serial);
        let owner = ObjectId::default();
        let hash = bundle.calc_hash_value();
        let len = bundle.len();
        let chunk_list = ChunkList::ChunkInBundle(bundle);

        let file = cyfs_base::File::new(owner.clone(), len, hash.clone(), chunk_list)
            .no_create_time()
            .build();
        file
    }

    fn query_server_exists_chunks_from_cache(&self, chunk_list: Vec<ChunkId>) -> Vec<ChunkId> {
        chunk_list
            .into_iter()
            .filter_map(|chunk_id| {
                if self.state_cache.is_object_missing(chunk_id.as_object_id()) {
                    None
                } else {
                    Some(chunk_id)
                }
            })
            .collect()
    }

    async fn query_server_exists_chunks(
        &self,
        chunk_list: Vec<ChunkId>,
    ) -> BuckyResult<Vec<ChunkId>> {
        assert!(chunk_list.len() > 0);

        // first filter the none exists items from the local cache state
        let chunk_list = self.query_server_exists_chunks_from_cache(chunk_list);
        if chunk_list.is_empty() {
            return Ok(chunk_list);
        }

        // then filter from server
        let req = SyncChunksRequest {
            chunk_list,
            states: vec![ChunkState::Ready],
        };

        let resp = self.requestor.sync_chunks(&req).await?;
        assert_eq!(resp.result.len(), req.chunk_list.len());

        let mut result = vec![];
        let mut missing = vec![];
        req.chunk_list
            .into_iter()
            .zip(resp.result.into_iter())
            .for_each(|(chunk_id, exists)| {
                if exists {
                    result.push(chunk_id);
                } else {
                    self.state_cache.miss_object(chunk_id.as_object_id());
                    missing.push(chunk_id);
                }
            });

        if missing.len() > 0 {
            info!("sync chunk from server but not exists: {:?}", missing);
        }

        Ok(result)
    }

    pub async fn sync_chunks(&self, chunk_list: Vec<ChunkId>) -> BuckyResult<()> {
        // remove the chunks already exists in local
        let chunk_list = self.filter_exists_chunks(chunk_list).await?;
        if chunk_list.is_empty() {
            return Ok(());
        }

        // only sync chunks exists on server side
        let chunk_list = self.query_server_exists_chunks(chunk_list).await?;
        if chunk_list.is_empty() {
            return Ok(());
        }

        // TODO Balance between single chunk and bundle file mode
        self.sync_chunks_with_single_chunk(chunk_list).await
    }

    // in single chunk mode
    async fn sync_chunks_with_single_chunk(&self, chunk_list: Vec<ChunkId>) -> BuckyResult<()> {
        for chunk_id in chunk_list {
            match self.sync_single_chunk(&chunk_id).await {
                Ok(()) => continue,
                Err(e) => match e.code() {
                    BuckyErrorCode::NotFound => {
                        self.state_cache.miss_object(chunk_id.as_object_id());
                    }
                    _ => {
                        error!("sync single chunk but failed! now will stop sync chunk list! chunk={}, {}", chunk_id, e);
                        return Err(e);
                    }
                },
            }
        }

        Ok(())
    }

    async fn sync_single_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<()> {
        info!("will sync single chunk, chunk={}", chunk_id);

        let task_id = format!("sync_chunk_{}", chunk_id);

        let context = self
            .named_data_components
            .context_manager
            .create_download_context_from_target("", self.ood_device_id.clone())
            .await?;

        let writer = self.named_data_components.new_chunk_writer();

        let (id, reader) =
            cyfs_bdt::download_chunk(&self.bdt_stack, chunk_id.to_owned(), None, context)
                .await
                .map_err(|e| {
                    error!(
                        "start bdt chunk sync session error! task_id={}, {}",
                        task_id, e
                    );
                    e
                })?;

        let adapter = ChunkListReaderAdapter::new_chunk(Arc::new(writer), reader, &chunk_id);

        match adapter.run().await {
            Ok(()) => {
                info!("sync single chunk success! chunk={}, task={}", chunk_id, id);
                Ok(())
            }
            Err(e) => {
                warn!(
                    "sync single chunk failed! chunk={}, task={}, {}",
                    chunk_id, id, e
                );
                Err(e)
            }
        }
    }

    // in bundle file mode
    async fn sync_chunks_with_file(&self, chunk_list: Vec<ChunkId>) -> BuckyResult<()> {
        let count = chunk_list.len();

        // create a bundle file to download the chunks
        let file = Self::create_file(chunk_list);
        let file_id = file.desc().calculate_id();

        info!(
            "will sync chunks as file, count={}, file={}",
            count, file_id
        );

        let task_id = format!("sync_chunks_{}", file_id);

        let context = self
            .named_data_components
            .context_manager
            .create_download_context_from_target("", self.ood_device_id.clone())
            .await?;

        let writer = self.named_data_components.new_chunk_writer();

        let (id, reader) = cyfs_bdt::download_file(&self.bdt_stack, file.clone(), None, context)
            .await
            .map_err(|e| {
                error!(
                    "start bdt chunks sync session error! task_id={}, {}",
                    task_id, e
                );
                e
            })?;

        let adapter = ChunkListReaderAdapter::new_file(Arc::new(writer), reader, &file);
        match adapter.run().await {
            Ok(()) => {
                info!("sync chunks success! file={}, task={}", file_id, id);
                Ok(())
            }
            Err(e) => {
                error!("sync chunks failed! file={}, task={}, {}", file_id, id, e);
                Err(e)
            }
        }
    }
}
