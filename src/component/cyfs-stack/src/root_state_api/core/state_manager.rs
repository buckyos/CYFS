use super::cache::*;
use super::root::*;
use super::root_index::RootInfo;
use crate::config::StackGlobalConfig;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::ReenterCallManager;

use async_std::sync::Mutex as AsyncMutex;
use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalStateManager {
    category: GlobalStateCategory,

    owner: Option<ObjectId>,

    global_root_state: GlobalStateRootRef,

    noc_cache: ObjectMapNOCCacheRef,

    root_list: Arc<AsyncMutex<HashMap<ObjectId, ObjectMapRootManagerRef>>>,

    create_root_manager_reenter_call_manager:
        ReenterCallManager<ObjectId, BuckyResult<ObjectMapRootManagerRef>>,
}

impl GlobalStateManager {
    pub async fn load(
        category: GlobalStateCategory,
        device_id: &DeviceId,
        owner: Option<ObjectId>,
        noc: Box<dyn NamedObjectCache>,
        config: StackGlobalConfig,
    ) -> BuckyResult<Self> {
        let noc_cache = ObjectMapNOCCacheAdapter::new_noc_cache(&device_id, noc.clone_noc());
        let global_root_state = GlobalStateRoot::load(
            category.clone(),
            device_id,
            owner,
            noc,
            noc_cache.clone(),
            config,
        )
        .await?;
        let global_root_state = Arc::new(global_root_state);

        let ret = Self {
            category,
            owner,
            global_root_state,
            noc_cache,
            root_list: Arc::new(AsyncMutex::new(HashMap::new())),
            create_root_manager_reenter_call_manager: ReenterCallManager::new(),
        };

        Ok(ret)
    }

    pub fn access_mode(&self) -> GlobalStateAccessMode {
        self.global_root_state.access_mode()
    }

    // direct change the root
    pub(crate) async fn direct_set_root_state(
        &self,
        new_root_info: RootInfo,
        prev_root_id: Option<ObjectId>,
    ) -> BuckyResult<()> {
        info!("will direct set root state: category={}, {:?} -> {:?}", self.category, prev_root_id, new_root_info);

        // should keep the lock during the whole func
        // Prevent inconsistencies in the instantaneous state caused by the successive setting of global_root and dec_root
        let mut root_list_holder = self.root_list.lock().await;

        self.global_root_state
            .direct_set_root_state(new_root_info, prev_root_id)
            .await?;

        // 尝试更新所有已经加载的dec_root
        let list: Vec<(ObjectId, ObjectMapRootManagerRef)> = root_list_holder
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect();

        for (dec_id, root_manager) in list {
            match self.global_root_state.get_dec_root(&dec_id, false).await {
                Ok(Some(root_info)) => {
                    if root_info.dec_root != root_manager.get_current_root() {
                        root_manager
                            .root_holder()
                            .direct_reload_root(root_info.dec_root)
                            .await;
                    }
                }
                Ok(None) => {
                    warn!(
                        "dec root had been removed! now will remove dec root manager: dec={}",
                        dec_id
                    );
                    root_list_holder.remove(&dec_id);
                }
                Err(e) => {
                    warn!(
                        "got dec root error! now will remove dec root manager: dec={}, {}",
                        dec_id, e
                    );
                    root_list_holder.remove(&dec_id);
                }
            }
        }

        Ok(())
    }

    // return (global_root, revision,)
    pub fn get_current_root(&self) -> (ObjectId, u64) {
        self.global_root_state.get_current_root()
    }

    pub fn get_root_revision(&self, root: &ObjectId) -> Option<u64> {
        self.global_root_state.revision().get_root_revision(root)
    }

    pub fn root_cache(&self) -> &ObjectMapRootCacheRef {
        self.global_root_state.root_cache()
    }

    // return (global_root, revision, dec_root)
    pub async fn get_dec_root(
        &self,
        dec_id: &ObjectId,
    ) -> BuckyResult<Option<(ObjectId, u64, ObjectId)>> {
        let ret = self.global_root_state.get_dec_root(dec_id, false).await?;
        match ret {
            Some(info) => {
                let revision = self
                    .global_root_state
                    .revision()
                    .get_root_revision(&info.root)
                    .unwrap();
                Ok(Some((info.root, revision, info.dec_root)))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn get_dec_relation_root_info(&self, dec_root: &ObjectId) -> (ObjectId, u64) {
        self.global_root_state
            .revision()
            .get_dec_relation_root_info(dec_root)
    }

    pub async fn get_dec_root_manager(
        &self,
        dec_id: &ObjectId,
        auto_create: bool,
    ) -> BuckyResult<ObjectMapRootManagerRef> {
        {
            let root_list = self.root_list.lock().await;
            let root = root_list.get(dec_id);
            if root.is_some() {
                return Ok(root.unwrap().clone());
            }
        }

        /*
        if !auto_create {
            let msg = format!(
                "get dec_root_state but not found! category={}, dec={}",
                self.category, dec_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }
        */

        // 同一个dec的并发调用需要防重入
        let this = self.clone();
        let owned_dec_id = dec_id.to_owned();
        self.create_root_manager_reenter_call_manager
            .call(&dec_id, async move {
                this.create_dec_root_manager(&owned_dec_id, auto_create)
                    .await
            })
            .await
    }

    async fn create_dec_root_manager(
        &self,
        dec_id: &ObjectId,
        auto_create: bool,
    ) -> BuckyResult<ObjectMapRootManagerRef> {
        // TODO 这里需要有防重入机制
        // 创建
        let root = self.create_dec_root(dec_id, auto_create).await?;
        let mut root = Arc::new(root);

        {
            let mut root_list = self.root_list.lock().await;
            match root_list.entry(dec_id.to_owned()) {
                Entry::Vacant(v) => {
                    v.insert(root.clone());
                }
                Entry::Occupied(o) => {
                    info!(
                        "create root for dec but already created! category={}, dec={}",
                        self.category, dec_id
                    );
                    root = o.get().clone();
                }
            }
        }

        Ok(root)
    }

    async fn create_dec_root(
        &self,
        dec_id: &ObjectId,
        auto_create: bool,
    ) -> BuckyResult<ObjectMapRootManager> {
        info!(
            "will load root for dec: category={}, dec={}, auto_create={}",
            self.category, dec_id, auto_create,
        );

        let root_info = self
            .global_root_state
            .get_dec_root(dec_id, auto_create)
            .await?;

        if root_info.is_none() {
            let msg = format!(
                "get dec_root but not found! category={}, dec={},",
                self.category, dec_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }
        let root_info = root_info.unwrap();

        info!(
            "will create root manager for dec: category={}, dec={}, root_info={:?}",
            self.category, dec_id, root_info
        );
        let event = Arc::new(Box::new(self.clone()) as Box<dyn ObjectMapRootEvent>);
        let root_holder =
            ObjectMapRootHolder::new(Some(dec_id.to_owned()), root_info.dec_root, event);

        let root_manager = ObjectMapRootManager::new(
            self.owner.clone(),
            Some(dec_id.to_owned()),
            self.noc_cache.clone(),
            root_holder,
        );
        Ok(root_manager)
    }
}

#[async_trait::async_trait]
impl ObjectMapRootEvent for GlobalStateManager {
    async fn root_updated(
        &self,
        dec_id: &Option<ObjectId>,
        new_root_id: ObjectId,
        prev_id: ObjectId,
    ) -> BuckyResult<()> {
        assert!(dec_id.is_some());

        self.global_root_state
            .update_dec_root(dec_id.as_ref().unwrap(), new_root_id, prev_id)
            .await?;

        Ok(())
    }
}
