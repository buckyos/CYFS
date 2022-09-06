use super::dec_manager::*;
use super::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex};

pub struct GlobalStatePathMetaItem {
    manager: GlobalStateDecPathMetaManagerRef,
    last_access: u64,
}

pub struct GlobalStatePathMetaManager {
    isolate: String,
    root_state: GlobalStateOutputProcessorRef,
    local_cache: GlobalStateOutputProcessorRef,
    noc: Arc<Box<dyn NamedObjectCache>>,

    all: Mutex<HashMap<ObjectId, GlobalStatePathMetaItem>>,
}

impl GlobalStatePathMetaManager {
    pub fn new(
        isotate: &str,
        root_state: GlobalStateOutputProcessorRef,
        local_cache: GlobalStateOutputProcessorRef,
        noc: Arc<Box<dyn NamedObjectCache>>,
    ) -> Self {
        Self {
            isolate: isotate.to_owned(),
            root_state,
            local_cache,
            noc,
            all: Mutex::new(HashMap::new()),
        }
    }

    fn new_dec_meta(&self, dec_id: ObjectId) -> GlobalStateDecPathMetaManagerRef {
        let raw = GlobalStateDecPathMetaManager::new(
            &self.isolate,
            self.root_state.clone(),
            self.local_cache.clone(),
            Some(dec_id),
            self.noc.clone(),
        );

        Arc::new(raw)
    }

    fn get_dec_meta(
        &self,
        dec_id: &Option<ObjectId>,
        auto_create: bool,
    ) -> Option<GlobalStateDecPathMetaManagerRef> {
        let dec_id = match dec_id {
            Some(id) => id,
            None => cyfs_core::get_system_dec_app().object_id(),
        };

        if auto_create {
            let mut list = self.all.lock().unwrap();
            match list.entry(dec_id.to_owned()) {
                Entry::Occupied(mut o) => {
                    let item = o.get_mut();
                    item.last_access = bucky_time_now();
                    Some(item.manager.clone())
                }
                Entry::Vacant(v) => {
                    let manager = self.new_dec_meta(v.key().to_owned());
                    let item = GlobalStatePathMetaItem {
                        manager: manager.clone(),
                        last_access: bucky_time_now(),
                    };

                    v.insert(item);
                    Some(manager)
                }
            }
        } else {
            let mut list = self.all.lock().unwrap();
            match list.get_mut(&dec_id) {
                Some(item) => {
                    item.last_access = bucky_time_now();
                    Some(item.manager.clone())
                }
                None => None,
            }
        }
    }

    pub async fn get_global_state_meta(
        &self,
        category: GlobalStateCategory,
        dec_id: Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        match category {
            GlobalStateCategory::RootState => self.get_root_state_meta(dec_id, auto_create).await,
            GlobalStateCategory::LocalCache => self.get_local_cache_meta(dec_id, auto_create).await,
        }
    }

    pub async fn get_root_state_meta(
        &self,
        dec_id: Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        let ret = self.get_dec_meta(&dec_id, auto_create);
        if ret.is_none() {
            let msg = format!("root state path meta from dec not found! dec={:?}", dec_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let manager = ret.unwrap();
        manager.get_root_state_meta().await
    }

    pub async fn get_local_cache_meta(
        &self,
        dec_id: Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        let ret = self.get_dec_meta(&dec_id, auto_create);
        if ret.is_none() {
            let msg = format!("local cache path meta from dec not found! dec={:?}", dec_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let manager = ret.unwrap();
        manager.get_local_cache_meta().await
    }
}

pub type GlobalStatePathMetaManagerRef = Arc<GlobalStateDecPathMetaManager>;
