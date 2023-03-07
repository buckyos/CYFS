use crate::meta::ObjectArchiveDecMeta;
use crate::data::BackupDataWriterRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ObjectArchiveDecMetaHolder {
    meta: Arc<Mutex<ObjectArchiveDecMeta>>,
}

impl ObjectArchiveDecMetaHolder {
    pub fn new(dec_id: ObjectId, dec_root: ObjectId) -> Self {
        Self {
            meta: Arc::new(Mutex::new(ObjectArchiveDecMeta::new(dec_id, dec_root))),
        }
    }

    pub fn into_inner(self) -> ObjectArchiveDecMeta {
        let item = Arc::try_unwrap(self.meta).unwrap();
        item.into_inner().unwrap()
    }

    fn on_error(&self, id: &ObjectId) {
        let mut meta = self.meta.lock().unwrap();
        meta.meta.on_error(id)
    }

    fn on_missing(&self, id: &ObjectId) {
        let mut meta = self.meta.lock().unwrap();
        meta.meta.on_missing(id)
    }

    fn on_object(&self, object: &NONObjectInfo) {
        let mut meta = self.meta.lock().unwrap();
        meta.meta.on_object(object.object_raw.len());
    }

    fn on_chunk(&self, chunk_id: &ChunkId) {
        let mut meta = self.meta.lock().unwrap();
        meta.meta.on_chunk(chunk_id)
    }
}

#[derive(Clone)]
pub struct ObjectTraverserHelper {
    isolate_id: Option<ObjectId>,
    dec_id: Option<ObjectId>,

    // archive: ObjectArchiveGenerator,
    backup_meta: ObjectArchiveDecMetaHolder,

    data_writer: BackupDataWriterRef,
    loader: ObjectTraverserLoaderRef,
    dec_meta: Option<GlobalStateMetaRawProcessorRef>,
}

impl ObjectTraverserHelper {
    pub fn new(
        isolate_id: Option<ObjectId>,
        dec_id: Option<ObjectId>,
        backup_meta: ObjectArchiveDecMetaHolder,

        data_writer: BackupDataWriterRef,
        loader: ObjectTraverserLoaderRef,
        dec_meta: Option<GlobalStateMetaRawProcessorRef>,
    ) -> Self {
        Self {
            isolate_id,
            dec_id,

            backup_meta,
            data_writer,
            loader,
            dec_meta,
        }
    }

    pub async fn run(&self, root: &ObjectId) -> BuckyResult<()> {
        let handler = self.clone_handler();
        let traverser = ObjectTraverser::new(self.loader.clone(), handler);
        traverser.run(root.clone()).await.map_err(|e| {
            let msg = format!(
                "backup dec root failed! dec={:?}, root={}, {}",
                self.dec_id, root, e
            );
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        Ok(())
    }

    fn clone_handler(&self) -> ObjectTraverserHandlerRef {
        Arc::new(Box::new(self.clone()))
    }
}

#[async_trait::async_trait]
impl ObjectTraverserHandler for ObjectTraverserHelper {
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
                        "dec's state path is virtual: dec={:?}, path={}",
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

    async fn on_error(&self, id: &ObjectId, e: BuckyError) -> BuckyResult<()> {
        self.backup_meta.on_error(id);

        self.data_writer.on_error(self.isolate_id.as_ref(), self.dec_id.as_ref(), id, e).await
    }

    async fn on_missing(&self, id: &ObjectId) -> BuckyResult<()> {
        self.backup_meta.on_missing(id);

        self.data_writer.on_missing(self.isolate_id.as_ref(), self.dec_id.as_ref(), id).await
    }

    async fn on_object(
        &self,
        object: &NONObjectInfo,
        meta: &Option<NamedObjectMetaData>,
    ) -> BuckyResult<()> {
        self.backup_meta.on_object(object);

        self.data_writer
            .add_object(&object.object_id, &object.object_raw, meta.as_ref())
            .await
    }

    async fn on_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<()> {
        self.backup_meta.on_chunk(chunk_id);

        match self.loader.get_chunk(chunk_id).await {
            Ok(Some(data)) => {
                self.data_writer
                    .add_chunk(chunk_id.to_owned(), data, None)
                    .await
            }
            Ok(None) => self.on_missing(chunk_id.as_object_id()).await,
            Err(e) => self.on_error(chunk_id.as_object_id(), e).await,
        }
    }
}
