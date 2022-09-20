use super::super::client::SyncClientRequestor;
use super::super::protocol::SyncChunksRequest;
use super::cache::SyncObjectsStateCache;
use super::dir_sync::DirListSync;
use crate::ndn_api::ChunkManagerWriter;
use cyfs_base::*;
use cyfs_bdt::*;
use cyfs_chunk_cache::ChunkManager;
use cyfs_lib::*;

use futures::future::{AbortHandle, AbortRegistration, Abortable};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

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

// 等待错误发生，或者完成第一个chunk后返回
struct WaitWriterImpl {
    task_id: String,
    waker: Option<AbortHandle>,
    abort_registration: Option<AbortRegistration>,
    error: Option<BuckyError>,
}

#[derive(Clone)]
pub struct WaitWriter(Arc<Mutex<WaitWriterImpl>>);

impl WaitWriter {
    pub fn new(task_id: String) -> Self {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let imp = WaitWriterImpl {
            task_id,
            waker: Some(abort_handle),
            abort_registration: Some(abort_registration),
            error: None,
        };

        Self(Arc::new(Mutex::new(imp)))
    }

    pub async fn wait_and_return(&self) -> BuckyResult<()> {
        let abort_registration = self.0.lock().unwrap().abort_registration.take().unwrap();

        // 等待唤醒
        let future = Abortable::new(async_std::future::pending::<()>(), abort_registration);
        future.await.unwrap_err();

        if let Some(e) = self.0.lock().unwrap().error.take() {
            Err(e)
        } else {
            Ok(())
        }
    }

    fn try_wakeup(&self, err: Option<BuckyErrorCode>) {
        let waker = {
            let mut item = self.0.lock().unwrap();
            item.error = err.map(|code| BuckyError::from(code));
            item.waker.take()
        };

        if let Some(waker) = waker {
            debug!(
                "sync chunks wakeup writer will wake! {}, err={:?}",
                self.0.lock().unwrap().task_id,
                err
            );
            waker.abort();
        }
    }

    pub fn into_writer(self) -> Box<dyn ChunkWriter> {
        Box::new(self) as Box<dyn ChunkWriter>
    }
}

#[async_trait::async_trait]
impl ChunkWriter for WaitWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone()) as Box<dyn ChunkWriter>
    }

    async fn write(&self, _chunk_id: &ChunkId, _content: Arc<Vec<u8>>) -> BuckyResult<()> {
        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        self.try_wakeup(None);
        Ok(())
    }

    async fn err(&self, err: BuckyErrorCode) -> BuckyResult<()> {
        self.try_wakeup(Some(err));
        Ok(())
    }
}

impl std::fmt::Display for WaitWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let this = self.0.lock().unwrap();
        write!(f, "task: {}, ", this.task_id)?;
        if let Some(e) = &this.error {
            write!(f, ", err: {}, ", e)?;
        }
        write!(f, ", waked: {}, ", this.waker.is_none())?;

        Ok(())
    }
}

pub(super) struct DataSync {
    bdt_stack: StackGuard,
    chunk_manager: Arc<ChunkManager>,
    ood_device_id: DeviceId,
    requestor: Arc<SyncClientRequestor>,
    state_cache: SyncObjectsStateCache,
}

impl DataSync {
    pub fn new(
        bdt_stack: StackGuard,
        chunk_manager: Arc<ChunkManager>,
        requestor: Arc<SyncClientRequestor>,
        state_cache: SyncObjectsStateCache,
    ) -> Self {
        let ood_device_id = requestor.requestor().remote_device().unwrap();

        Self {
            bdt_stack,
            chunk_manager,
            ood_device_id,
            requestor,
            state_cache,
        }
    }

    pub fn create_dir_sync(&self) -> DirListSync {
        DirListSync::new(
            self.state_cache.clone(),
            self.bdt_stack.clone(),
            self.chunk_manager.clone(),
        )
    }

    async fn filter_exists_chunks(&self, chunk_list: Vec<ChunkId>) -> BuckyResult<Vec<ChunkId>> {
        let ndc = self.bdt_stack.ndn().chunk_manager().ndc();
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

        let context = SingleDownloadContext::streams(None, vec![self.ood_device_id.clone()]);
        let writer = Box::new(ChunkManagerWriter::new(
            self.chunk_manager.clone(),
            self.bdt_stack.ndn().chunk_manager().ndc().clone(),
            self.bdt_stack.ndn().chunk_manager().tracker().clone(),
        ));

        // used for waiting task finish or error
        let waiter = WaitWriter::new(task_id.clone());

        let _controller = cyfs_bdt::download::download_chunk(
            &self.bdt_stack,
            chunk_id.to_owned(),
            None, 
            Some(context),
            vec![writer, Box::new(waiter.clone())],
        )
        .await
        .map_err(|e| {
            error!(
                "start bdt chunk sync session error! task_id={}, {}",
                task_id, e
            );
            e
        })?;

        match waiter.wait_and_return().await {
            Ok(()) => {
                info!("sync single chunk success! chunk={}", chunk_id);
                Ok(())
            }
            Err(e) => {
                warn!("sync single chunk failed! chunk={}, {}", chunk_id, e);
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

        let context = SingleDownloadContext::streams(None, vec![self.ood_device_id.clone()]);
        let writer = Box::new(ChunkManagerWriter::new(
            self.chunk_manager.clone(),
            self.bdt_stack.ndn().chunk_manager().ndc().clone(),
            self.bdt_stack.ndn().chunk_manager().tracker().clone(),
        ));

        // used for waiting task finish or error
        let waiter = WaitWriter::new(task_id.clone());

        let _controller = cyfs_bdt::download::download_file(
            &self.bdt_stack,
            file,
            None, 
            Some(context),
            vec![writer, Box::new(waiter.clone())],
        )
        .await
        .map_err(|e| {
            error!(
                "start bdt chunks sync session error! task_id={}, {}",
                task_id, e
            );
            e
        })?;

        match waiter.wait_and_return().await {
            Ok(()) => {
                info!("sync chunks success! file={}", file_id);
                Ok(())
            }
            Err(e) => {
                error!("sync chunks failed! file={}, {}", file_id, e);
                Err(e)
            }
        }
    }
}
