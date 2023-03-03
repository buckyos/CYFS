use super::data::BackupDataManager;
use crate::archive::ObjectArchiveDecMeta;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct ObjectArchiveDecMetaHolder {
    meta: Arc<Mutex<ObjectArchiveDecMeta>>,
}

impl ObjectArchiveDecMetaHolder {
    pub fn new(dec_id: ObjectId, dec_root: ObjectId) -> Self {
        Self {
            meta: Arc::new(Mutex::new(ObjectArchiveDecMeta::new(dec_id, dec_root))),
        }
    }

    fn into_inner(self) -> ObjectArchiveDecMeta {
        let item = Arc::try_unwrap(self.meta).unwrap();
        item.into_inner().unwrap()
    }

    fn on_error(&self, id: &ObjectId, _e: BuckyError) {
        let mut meta = self.meta.lock().unwrap();
        if id.is_chunk_id() {
            let chunk_id = id.as_chunk_id();
            meta.error.chunks.bytes += chunk_id.len() as u64;
            meta.error.chunks.count += 1;
        } else {
            meta.error.objects.count += 1;
        }
    }

    fn on_missing(&self, id: &ObjectId) {
        let mut meta = self.meta.lock().unwrap();
        if id.is_chunk_id() {
            let chunk_id = id.as_chunk_id();
            meta.missing.chunks.bytes += chunk_id.len() as u64;
            meta.missing.chunks.count += 1;
        } else {
            meta.missing.objects.count += 1;
        }
    }

    fn on_object(&self, object: &NONObjectInfo) {
        let mut meta = self.meta.lock().unwrap();

        meta.data.objects.count += 1;
        meta.data.objects.bytes += object.object_raw.len() as u64;
    }

    fn on_chunk(&self, chunk_id: &ChunkId) {
        let mut meta = self.meta.lock().unwrap();

        meta.data.chunks.bytes += chunk_id.len() as u64;
        meta.data.chunks.count += 1;
    }
}

#[derive(Clone)]
pub struct DecStateBackup {
    dec_id: ObjectId,
    dec_root: ObjectId,

    // archive: ObjectArchiveGenerator,
    backup_meta: ObjectArchiveDecMetaHolder,

    data_manager: BackupDataManager,
    loader: ObjectTraverserLoaderRef,
    dec_meta: Option<GlobalStateMetaRawProcessorRef>,
}

impl DecStateBackup {
    pub fn new(
        dec_id: ObjectId,
        dec_root: ObjectId,
        data_manager: BackupDataManager,
        loader: ObjectTraverserLoaderRef,
        dec_meta: Option<GlobalStateMetaRawProcessorRef>,
    ) -> Self {
        Self {
            backup_meta: ObjectArchiveDecMetaHolder::new(dec_id.clone(), dec_root.clone()),
            dec_id,
            dec_root,
            data_manager,
            loader,
            dec_meta,
        }
    }

    pub async fn run(self) -> BuckyResult<ObjectArchiveDecMeta> {
        let handler = self.clone_handler();
        let traverser = ObjectTraverser::new(self.loader, handler);
        traverser.run(self.dec_id.clone()).await.map_err(|e| {
            let msg = format!(
                "backup dec failed! dec={}, root={}, {}",
                self.dec_id, self.dec_root, e
            );
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        drop(traverser);

        Ok(self.backup_meta.into_inner())
    }

    fn clone_handler(&self) -> ObjectTraverserHandlerRef {
        Arc::new(Box::new(self.clone()))
    }
}

#[async_trait::async_trait]
impl ObjectTraverserHandler for DecStateBackup {
    async fn filter_path(&self, path: &str) -> ObjectTraverseFilterResult {
        if self.dec_meta.is_none() {
            return ObjectTraverseFilterResult::Keep(None);
        }

        match self.dec_meta.as_ref().unwrap().query_path_config(path) {
            Some(meta) => match meta.storage_state {
                None | Some(GlobalStatePathStorageState::Concrete) => {
                    ObjectTraverseFilterResult::Keep(meta.depth.map(|v| v as u32))
                }
                Some(GlobalStatePathStorageState::Virtual) => {
                    debug!(
                        "dec's state path is virtual: dec={}, path={}",
                        self.dec_id, path
                    );
                    ObjectTraverseFilterResult::Skip
                }
            },
            None => ObjectTraverseFilterResult::Keep(None),
        }
    }

    async fn filter_object(
        &self,
        object: &NONObjectInfo,
        meta: Option<&NamedObjectMetaData>,
    ) -> ObjectTraverseFilterResult {
        if self.dec_meta.is_none() {
            return ObjectTraverseFilterResult::Keep(None);
        }

        let provider = match meta {
            Some(meta) => meta as &dyn ObjectSelectorDataProvider,
            None => object as &dyn ObjectSelectorDataProvider,
        };

        match self.dec_meta.as_ref().unwrap().query_object_meta(provider) {
            Some(meta) => ObjectTraverseFilterResult::Keep(meta.depth.map(|v| v as u32)),
            None => ObjectTraverseFilterResult::Keep(None),
        }
    }

    async fn on_error(&self, id: &ObjectId, e: BuckyError) {
        self.backup_meta.on_error(id, e);
    }

    async fn on_missing(&self, id: &ObjectId) {
        self.backup_meta.on_missing(id);
    }

    async fn on_object(&self, object: &NONObjectInfo, meta: &Option<NamedObjectMetaData>) {
        self.backup_meta.on_object(object);

        self.data_manager
            .add_object(&object.object_id, &object.object_raw, meta.as_ref())
            .await;
    }

    async fn on_chunk(&self, chunk_id: &ChunkId) {
        self.backup_meta.on_chunk(chunk_id);

        match self.loader.get_chunk(chunk_id).await {
            Ok(Some(data)) => {
                self.data_manager
                    .add_data(chunk_id.object_id(), data, None)
                    .await;
            }
            Ok(None) => {
                self.on_missing(chunk_id.as_object_id()).await;
            }
            Err(e) => {
                self.on_error(chunk_id.as_object_id(), e).await;
            }
        }
    }
}
