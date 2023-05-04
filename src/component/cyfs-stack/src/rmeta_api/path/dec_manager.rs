use super::meta::*;
use super::storage::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::ReenterCallManager;

use once_cell::sync::OnceCell;
use std::sync::Arc;

struct GlobalStateDecPathMetaHolder {
    root_state: GlobalStateOutputProcessorRef,
    category: GlobalStateCategory,
    dec_id: Option<ObjectId>,
    meta: Arc<OnceCell<BuckyResult<GlobalStatePathMetaSyncCollection>>>,
    noc: NamedObjectCacheRef,
    device_id: DeviceId,

    storage: Arc<GlobalStatePathMetaStorage>,

    load_reenter_manager: ReenterCallManager<(), ()>,
}

impl GlobalStateDecPathMetaHolder {
    pub fn new(
        isolate: &str,
        root_state: GlobalStateOutputProcessorRef,
        category: GlobalStateCategory,
        dec_id: Option<ObjectId>,
        noc: NamedObjectCacheRef,
        device_id: DeviceId,
    ) -> Self {
        let storage = GlobalStatePathMetaStorage::new(isolate, &dec_id);

        Self {
            root_state,
            category,
            dec_id,
            meta: Arc::new(OnceCell::new()),
            noc,
            device_id,
            storage: Arc::new(storage),
            load_reenter_manager: ReenterCallManager::new(),
        }
    }

    fn get(&self) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        match self.meta.get().unwrap() {
            Ok(v) => Ok(v.clone()),
            Err(e) => Err(e.to_owned()),
        }
    }

    pub async fn get_or_load(&self) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        if let Some(_) = self.meta.get() {
            return self.get();
        }

        let root_state = self.root_state.clone();
        let category = self.category.clone();
        let dec_id = self.dec_id.clone();
        let noc = self.noc.clone();
        let meta = self.meta.clone();
        let storage = self.storage.clone();
        let device_id = self.device_id.clone();

        self.load_reenter_manager
            .call(&(), async move {
                let ret = Self::load(root_state, category, dec_id, noc, storage, device_id).await;
                if let Err(_) = meta.set(ret) {
                    unreachable!();
                }

                ()
            })
            .await;

        self.get()
    }

    async fn load(
        root_state: GlobalStateOutputProcessorRef,
        category: GlobalStateCategory,
        dec_id: Option<ObjectId>,
        noc: NamedObjectCacheRef,
        storage: Arc<GlobalStatePathMetaStorage>,
        device_id: DeviceId,
    ) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        let meta_path = format!("{}/{}", CYFS_GLOBAL_STATE_META_PATH, category.as_str());

        let id = match category {
            GlobalStateCategory::RootState => "cyfs-root-state-path-meta",
            GlobalStateCategory::LocalCache => "cyfs-local-cache-path-meta",
        };

        let data = NOCCollectionRWAsync::<GlobalStatePathMeta>::new_global_state(
            root_state,
            dec_id.clone(),
            meta_path,
            None,
            id,
            noc.clone(),
        );

        if let Err(e) = data.load().await {
            // FIXME 如果加载失败要如何处理，需要初始化为空还是直接返回错误终止执行？
            error!(
                "load global state path meta failed! dec={:?}, category={}, {}",
                dec_id, category, e,
            );

            return Err(e);
        }

        info!(
            "load global state meta success! dec={}, category={}, content={}",
            GlobalStatePathMetaStorage::get_dec_string(&dec_id),
            category,
            serde_json::to_string(&data.coll().read().await as &GlobalStatePathMeta).unwrap(),
        );

        let ret = GlobalStatePathMetaSyncCollection::new(device_id, storage, data);
        Ok(ret)
    }
}

pub struct GlobalStateDecPathMetaManager {
    dec_id: Option<ObjectId>,
    meta: GlobalStateDecPathMetaHolder,
}

impl GlobalStateDecPathMetaManager {
    pub fn new(
        isolate: &str,
        root_state: GlobalStateOutputProcessorRef,
        category: GlobalStateCategory,
        dec_id: Option<ObjectId>,
        noc: NamedObjectCacheRef,
        device_id: DeviceId,
    ) -> Self {
        let meta = GlobalStateDecPathMetaHolder::new(
            isolate,
            root_state,
            category,
            dec_id.clone(),
            noc.clone(),
            device_id,
        );

        Self { dec_id, meta }
    }

    pub async fn get_global_state_meta(&self) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        self.meta.get_or_load().await
    }
}

pub type GlobalStateDecPathMetaManagerRef = Arc<GlobalStateDecPathMetaManager>;
