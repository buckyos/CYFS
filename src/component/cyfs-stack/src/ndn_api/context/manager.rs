use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_bdt::*;

use lru_time_cache::LruCache;
use std::sync::{Arc, Mutex};

pub(crate) struct ContextItem {
    pub object: TransContext,
    pub source_list: Vec<DownloadSource>,
}

pub(crate) struct ContextManager {
    noc: NamedObjectCacheRef,
    device_manager: Box<dyn DeviceCache>,
    list: Arc<Mutex<LruCache<ObjectId, Arc<ContextItem>>>>,
}

impl ContextManager {
    pub fn new(noc: NamedObjectCacheRef, device_manager: Box<dyn DeviceCache>,) -> Self {
        Self {
            noc,
            device_manager,
            list: Arc::new(Mutex::new(LruCache::with_expiry_duration_and_capacity(
                std::time::Duration::from_secs(60 * 10),
                128,
            ))),
        }
    }

    async fn new_item(&self, object: TransContext) -> ContextItem {
        let mut source_list = Vec::with_capacity(object.device_list().len());
        for item in object.device_list() {
            let ret = self.device_manager.get(&item.target).await;
            if ret.is_none() {
                warn!("load trans context target but not found! context={}, target={}", object.context_path(), item.target);
                continue;
            }

            let device = ret.unwrap();
            let source = DownloadSource {
                target: device.into_desc(),
                encode_desc: item.chunk_codec_type.clone(),
            };
            source_list.push(source);
        }

        ContextItem {
            object,
            source_list,
        }
    }

    pub async fn get_context(&self, id: &ObjectId) -> Option<Arc<ContextItem>> {
        let (ret, gc_list) = {
            let mut cache = self.list.lock().unwrap();
            let (ret, gc_list) = cache.notify_get(id);
            (ret.cloned(), gc_list)
        };

        if let Some(item) = ret {
            return Some(item.clone());
        }

        drop(gc_list);

        // then load from noc
        if let Ok(Some(object)) = self.load_context_from_noc(id).await {
            let item = self.new_item(object).await;
            let item = Arc::new(item);
            self.update_context(&id, item.clone());
            Some(item)
        } else {
            None
        }
    }

    async fn load_context_from_noc(&self, id: &ObjectId) -> BuckyResult<Option<TransContext>> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            object_id: id.to_owned(),
            source: RequestSourceInfo::new_local_system(),
            last_access_rpath: None,
        };

        match self.noc.get_object(&noc_req).await {
            Ok(Some(resp)) => {
                let object = TransContext::clone_from_slice(resp.object.object_raw.as_slice())
                    .map_err(|e| {
                        let msg = format!(
                            "get trans context from noc but invalid object! id={}, {}",
                            id, e
                        );
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::InvalidData, msg)
                    })?;

                Ok(Some(object))
            }
            Ok(None) => {
                warn!("get trans context object from noc but not found: id={}", id);
                Ok(None)
            }
            Err(e) => {
                warn!("get trans context object from noc failed! id={}, {}", id, e);
                Err(e)
            }
        }
    }

    pub async fn put_context(
        &self,
        source: RequestSourceInfo,
        object: NONObjectInfo,
    ) -> BuckyResult<()> {
        let trans_context = TransContext::clone_from_slice(&object.object_raw).map_err(|e| {
            let msg = format!(
                "invalid trans context object! id={}, {}",
                object.object_id, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // please make sure the id is matched before call this method!!
        // let id = trans_context.desc().calculate_id();
        let id = object.object_id.clone();

        let req = NamedObjectCachePutObjectRequest {
            source,
            object,
            storage_category: NamedObjectStorageCategory::Cache,
            context: None,
            last_access_rpath: None,
            access_string: None,
        };

        self.noc.put_object(&req).await.map_err(|e| {
            error!("save trans context to noc failed! id={}, {}", id, e);
            e
        })?;

        let item = self.new_item(trans_context).await;
        let item = Arc::new(item);
        self.update_context(&id, item);

        Ok(())
    }

    fn update_context(&self, id: &ObjectId, trans_context: Arc<ContextItem>) {
        let ret = {
            let mut cache = self.list.lock().unwrap();
            cache.notify_insert(id.clone(), trans_context)
        };

        match ret.0 {
            Some(v) => {
                info!("replace old trans context! id={}", id);
            }
            None => {}
        }
    }
}

pub(crate) type ContextManagerRef = Arc<ContextManager>;