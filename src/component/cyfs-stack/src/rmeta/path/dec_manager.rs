use super::meta::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::ReenterCallManager;

use once_cell::sync::OnceCell;
use std::sync::Arc;

const CYFS_GLOBAL_STATE_PATH_META: &str = ".cyfs/meta";

struct GlobalStateDecPathMetaHolder {
    global_state: GlobalStateOutputProcessorRef,
    category: GlobalStateCategory,
    dec_id: Option<ObjectId>,
    meta: Arc<OnceCell<BuckyResult<GlobalStatePathMetaSyncCollection>>>,
    noc: Arc<Box<dyn NamedObjectCache>>,

    load_reenter_manager: ReenterCallManager<(), ()>,
}

impl GlobalStateDecPathMetaHolder {
    pub fn new(
        global_state: GlobalStateOutputProcessorRef,
        category: GlobalStateCategory,
        dec_id: Option<ObjectId>,
        noc: Arc<Box<dyn NamedObjectCache>>,
    ) -> Self {
        Self {
            global_state,
            category,
            dec_id,
            meta: Arc::new(OnceCell::new()),
            noc,
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

        let global_state = self.global_state.clone();
        let category = self.category.clone();
        let dec_id = self.dec_id.clone();
        let noc = self.noc.clone();
        let meta = self.meta.clone();

        self.load_reenter_manager
            .call(&(), async move {
                let ret = Self::load(global_state, category, dec_id, noc).await;
                if let Err(_) = meta.set(ret) {
                    unreachable!();
                }

                ()
            })
            .await;

        self.get()
    }

    async fn load(
        global_state: GlobalStateOutputProcessorRef,
        category: GlobalStateCategory,
        dec_id: Option<ObjectId>,
        noc: Arc<Box<dyn NamedObjectCache>>,
    ) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        let meta_path = format!("{}/{}", CYFS_GLOBAL_STATE_PATH_META, category.as_str());

        let id = match category {
            GlobalStateCategory::RootState => "cyfs-root-state-path-meta",
            GlobalStateCategory::LocalCache => "cyfs-local-cache-path-meta",
        };

        let data = NOCCollectionRWSync::<GlobalStatePathMeta>::new_global_state(
            global_state,
            dec_id.clone(),
            meta_path,
            None,
            id,
            noc.clone_noc(),
        );

        if let Err(e) = data.load().await {
            // FIXME 如果加载失败要如何处理，需要初始化为空还是直接返回错误终止执行？
            error!(
                "load global state path meta failed! dec={:?}, category={}, {}",
                dec_id, category, e,
            );

            return Err(e);
        }

        let ret = GlobalStatePathMetaSyncCollection::new(data);
        Ok(ret)
    }
}

pub struct GlobalStateDecPathMetaManager {
    dec_id: Option<ObjectId>,
    root_state: GlobalStateDecPathMetaHolder,
    local_cache: GlobalStateDecPathMetaHolder,
}

impl GlobalStateDecPathMetaManager {
    pub fn new(
        root_state: GlobalStateOutputProcessorRef,
        local_cache: GlobalStateOutputProcessorRef,
        dec_id: Option<ObjectId>,
        noc: Arc<Box<dyn NamedObjectCache>>,
    ) -> Self {
        let root_state = GlobalStateDecPathMetaHolder::new(
            root_state,
            GlobalStateCategory::RootState,
            dec_id.clone(),
            noc.clone(),
        );
        let local_cache = GlobalStateDecPathMetaHolder::new(
            local_cache,
            GlobalStateCategory::LocalCache,
            dec_id.clone(),
            noc,
        );

        Self {
            dec_id,
            root_state,
            local_cache,
        }
    }

    pub async fn get_root_state_meta(&self) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        self.root_state.get_or_load().await
    }

    pub async fn get_local_cache_meta(&self) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        self.local_cache.get_or_load().await
    }
}

pub type GlobalStateDecPathMetaManagerRef = Arc<GlobalStateDecPathMetaManager>;
