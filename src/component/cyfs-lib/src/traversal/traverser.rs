use super::adapter::ObjectMapNOCCacheTranverseAdapter;
use super::object::*;
use crate::*;
use cyfs_base::*;
use cyfs_util::AsyncReadWithSeek;

use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[async_trait::async_trait]
pub trait ObjectTraverserFilter {}

pub type ObjectTraverserFilterRef = Arc<Box<dyn ObjectTraverserFilter>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectTraverseFilterResult {
    Skip,
    Keep(Option<u32>),
}

#[async_trait::async_trait]
pub trait ObjectTraverserHandler: Send + Sync {
    async fn filter_path(&self, path: &str) -> ObjectTraverseFilterResult;
    async fn filter_object(&self, object: &NONObjectInfo, meta: Option<&NamedObjectMetaData>) -> ObjectTraverseFilterResult;

    async fn on_error(&self, id: &ObjectId, e: BuckyError);
    async fn on_missing(&self, id: &ObjectId);

    async fn on_object(&self, object: &NONObjectInfo);
    async fn on_chunk(&self, chunk_id: &ChunkId);
}

pub type ObjectTraverserHandlerRef = Arc<Box<dyn ObjectTraverserHandler>>;

pub struct ObjectTraverserLoaderObjectData {
    pub object: NONObjectInfo,
    pub meta: Option<NamedObjectMetaData>,
}

#[async_trait::async_trait]
pub trait ObjectTraverserLoader: Send + Sync {
    async fn get_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectTraverserLoaderObjectData>>;
    async fn get_chunk(
        &self,
        chunk_id: &ChunkId,
    ) -> BuckyResult<Option<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>>>;
}

pub type ObjectTraverserLoaderRef = Arc<Box<dyn ObjectTraverserLoader>>;

struct ObjectTraverserColls {
    index: HashSet<ObjectId>,
    pending_items: VecDeque<NormalObject>,
}

impl ObjectTraverserColls {
    pub fn new() -> Self {
        Self {
            index: HashSet::new(),
            pending_items: VecDeque::new(),
        }
    }
}

#[derive(Clone)]
pub struct ObjectTraverser {
    coll: Arc<Mutex<ObjectTraverserColls>>,

    loader: ObjectTraverserLoaderRef,
    handler: ObjectTraverserHandlerRef,

    objectmap_cache: ObjectMapRootCacheRef,
}

impl ObjectTraverser {
    pub fn new(loader: ObjectTraverserLoaderRef, handler: ObjectTraverserHandlerRef) -> Self {
        let cache = ObjectMapNOCCacheTranverseAdapter::new_noc_cache(loader.clone());
        let objectmap_cache = ObjectMapRootMemoryCache::new_default_ref(None, cache);

        Self {
            coll: Arc::new(Mutex::new(ObjectTraverserColls::new())),
            loader,
            handler,
            objectmap_cache,
        }
    }

    async fn tranverse(&self, root: ObjectId) -> BuckyResult<()> {
        assert_eq!(root.obj_type_code(), ObjectTypeCode::ObjectMap);

        let ret = self.loader.get_object(&root).await?;
        if ret.is_none() {
            warn!("root object missing! root={}", root);
            self.handler.on_missing(&root).await;
            return Ok(());
        }

        let data = ret.unwrap();
        self.handler.on_object(&data.object).await;

        let filter_ret = self.handler.filter_path("/").await;
        let config_ref_depth = match filter_ret {
            ObjectTraverseFilterResult::Keep(config_ref_depth) => config_ref_depth.unwrap_or(1),
            ObjectTraverseFilterResult::Skip => {
                return Ok(());
            }
        };

        let item = NormalObject {
            pos: NormalObjectPostion::Middle,
            path: "/".to_owned(),
            object: data.object.into(),
            config_ref_depth,
            ref_depth: 0,
        };
        self.append(item);

        let op_env_cache = ObjectMapOpEnvMemoryCache::new_ref(self.objectmap_cache.clone());
        let cb = Arc::new(Box::new(self.clone()) as Box<dyn ObjectTraverserCallBack>);

        loop {
            let next = self.fetch();
            if next.is_none() {
                break Ok(());
            }

            let item = next.unwrap();
            match item.object.object_id.obj_type_code() {
                ObjectTypeCode::ObjectMap => {
                    let traverser = ObjectMapTraverser::new(op_env_cache.clone(), item, cb.clone());
                    traverser.tranverse().await?;
                }
                _ => {
                    assert!(!item.object.is_empty());
                    /*
                    let data = match self.load_object(&item.object.object_id).await {
                        Ok(Some(info)) => info,
                        Ok(None) => {
                            self.handler.on_missing(&item.object.object_id).await;
                            continue;
                        }
                        Err(e) => {
                            self.handler.on_error(&item.object.object_id, e).await;
                            continue;
                        }
                    };

                    item.object = data.object.into();
                    */

                    let traverser = CommonObjectTraverser::new(item, cb.clone());
                    traverser.tranverse().await;

                    let item = traverser.finish();

                    match item.object.object_id.obj_type_code() {
                        ObjectTypeCode::File => {
                            let traverser = FileObjectTraverser::new(item, cb.clone());
                            traverser.tranverse().await;
                        }
                        ObjectTypeCode::Dir => {
                            let mut traverser = DirObjectTraverser::new(self.loader.clone(), item, cb.clone());
                            traverser.tranverse().await;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    async fn load_object(&self, id: &ObjectId) -> BuckyResult<Option<NONObjectInfo>> {
        match self.loader.get_object(&id).await {
            Ok(Some(data)) => Ok(Some(data.object)),
            Ok(None) => {
                warn!("get object but not exists! {}", id);
                Ok(None)
            }
            Err(e) => {
                error!("get object got error! {}, {}", id, e);
                Err(e)
            }
        }
    }

    fn dedup(&self, object_id: &ObjectId) -> bool {
        let mut coll = self.coll.lock().unwrap();
        !coll.index.insert(object_id.to_owned())
    }

    fn append(&self, item: NormalObject) {
        assert!(!item.object.is_empty());

        self.coll.lock().unwrap().pending_items.push_back(item);
    }

    fn fetch(&self) -> Option<NormalObject> {
        self.coll.lock().unwrap().pending_items.pop_front()
    }

    async fn process_object(&self) {}
}

#[async_trait::async_trait]
impl ObjectTraverserCallBack for ObjectTraverser {
    async fn on_object(&self, item: TraverseObjectItem) {
        let id = item.object_id();
        if !self.dedup(id) {
            return;
        }

        match self.loader.get_object(id).await {
            Ok(Some(data)) => {
                self.handler.on_object(&data.object).await;

                match item {
                    TraverseObjectItem::Normal(mut item) => {
                        assert!(item.object.is_empty());

                        let filter_ret = match item.pos {
                            NormalObjectPostion::Middle => {
                                self.handler.filter_path(&item.path).await
                            }
                            NormalObjectPostion::Leaf | NormalObjectPostion::Assoc => {
                                if item.ref_depth >= item.config_ref_depth {
                                    info!(
                                        "will skip object on ref_depth > config_ref_depth: {:?}",
                                        item
                                    );
                                    return;
                                }

                                self.handler.filter_object(&data.object, data.meta.as_ref()).await
                            }
                        };

                        match filter_ret {
                            ObjectTraverseFilterResult::Keep(config_ref_depth) => {
                                if let Some(config_ref_depth) = config_ref_depth {
                                    if config_ref_depth == 0 {
                                        info!("will skip object on filter's config_ref_depth is 0: {:?}", item);
                                        return;
                                    }

                                    item.config_ref_depth = config_ref_depth;
                                    if item.ref_depth >= item.config_ref_depth {
                                        info!("will skip object on ref_depth >= config_ref_depth: {:?}", item);
                                        return;
                                    }
                                }

                                item.object = data.object.into();
                                self.append(item);
                            }
                            ObjectTraverseFilterResult::Skip => {
                                info!("will skip object: {:?}", item);
                            }
                        }
                    }
                    TraverseObjectItem::Sub(_) => {
                        // do nothing
                    }
                }
            }
            Ok(None) => {
                self.handler.on_missing(id).await;
            }
            Err(e) => {
                self.handler.on_error(id, e).await;
            }
        }
    }

    async fn on_chunk(&self, item: TraverseChunkItem) {
        self.handler.on_chunk(&item.chunk_id).await;
    }

    async fn on_error(&self, id: &ObjectId, e: BuckyError) {
        self.handler.on_error(id, e).await;
    }

    async fn on_missing(&self, id: &ObjectId) {
        self.handler.on_missing(id).await
    }
}
