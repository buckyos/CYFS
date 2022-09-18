use super::super::path::*;
use crate::rmeta::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex};

pub struct GlobalStatePathMetaItem {
    manager: GlobalStateDecPathMetaManagerRef,
    last_access: u64,
}

#[derive(Clone)]
pub struct GlobalStatePathMetaManager {
    isolate: String,
    root_state: GlobalStateOutputProcessorRef,
    category: GlobalStateCategory,
    noc: NamedObjectCacheRef,

    all: Arc<Mutex<HashMap<ObjectId, GlobalStatePathMetaItem>>>,
}

impl GlobalStatePathMetaManager {
    pub fn new(
        isotate: &str,
        root_state: GlobalStateOutputProcessorRef,
        category: GlobalStateCategory,
        noc: NamedObjectCacheRef,
    ) -> Self {
        Self {
            isolate: isotate.to_owned(),
            root_state,
            category,
            noc,
            all: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn new_dec_meta(&self, dec_id: ObjectId) -> GlobalStateDecPathMetaManagerRef {
        let raw = GlobalStateDecPathMetaManager::new(
            &self.isolate,
            self.root_state.clone(),
            self.category,
            Some(dec_id),
            self.noc.clone(),
        );

        Arc::new(raw)
    }

    pub fn clone_processor(&self) -> GlobalStateMetaInputProcessorRef {
        Arc::new(Box::new(self.clone()))
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
        dec_id: &Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        let ret = self.get_dec_meta(&dec_id, auto_create);
        if ret.is_none() {
            let msg = format!(
                "global state path meta for dec not found! {}, dec={:?}",
                self.root_state.get_category(),
                dec_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let manager = ret.unwrap();
        manager.get_global_state_meta().await
    }

    fn get_dec_id(common: &MetaInputRequestCommon) -> &Option<ObjectId> {
        if common.target_dec_id.is_some() {
            &common.target_dec_id
        } else {
            &None
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateMetaInputProcessor for GlobalStatePathMetaManager {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessInputResponse> {
        let meta = self.get_global_state_meta(Self::get_dec_id(&req.common), true).await?;
        let updated = meta.add_access(req.item).await?;

        let resp = GlobalStateMetaAddAccessInputResponse { updated };
        Ok(resp)
    }

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessInputResponse> {
        let meta = self.get_global_state_meta(Self::get_dec_id(&req.common), false).await?;
        let item = meta.remove_access(req.item).await?;

        let resp = GlobalStateMetaRemoveAccessInputResponse { item };

        Ok(resp)
    }

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessInputResponse> {
        let meta = self.get_global_state_meta(Self::get_dec_id(&req.common), false).await?;
        let count = meta.clear_access().await? as u32;

        let resp = GlobalStateMetaClearAccessInputResponse { count };
        Ok(resp)
    }

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkInputResponse> {
        let meta = self.get_global_state_meta(Self::get_dec_id(&req.common), true).await?;
        let updated = meta.add_link(req.source, req.target).await?;

        let resp = GlobalStateMetaAddLinkInputResponse { updated };
        Ok(resp)
    }

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkInputResponse> {
        let meta = self.get_global_state_meta(Self::get_dec_id(&req.common), false).await?;
        let item = meta.remove_link(&req.source).await?;

        let resp = GlobalStateMetaRemoveLinkInputResponse { item };

        Ok(resp)
    }

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkInputResponse> {
        let meta = self.get_global_state_meta(Self::get_dec_id(&req.common), false).await?;
        let count = meta.clear_link().await? as u32;

        let resp = GlobalStateMetaClearLinkInputResponse { count };
        Ok(resp)
    }
}

pub type GlobalStatePathMetaManagerRef = Arc<GlobalStatePathMetaManager>;