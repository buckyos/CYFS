use super::super::path::*;
use crate::rmeta::*;
use crate::root_state_api::GlobalStateLocalService;
use cyfs_base::*;
use cyfs_lib::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;
use cyfs_debug::Mutex;

pub struct GlobalStatePathMetaItem {
    manager: GlobalStateDecPathMetaManagerRef,
    last_access: u64,
}

#[derive(Clone)]
pub struct GlobalStatePathMetaManager {
    isolate: String,
    root_state: GlobalStateOutputProcessorRef,
    root_state_service: GlobalStateLocalService,
    category: GlobalStateCategory,
    noc: NamedObjectCacheRef,
    device_id: DeviceId,

    all: Arc<Mutex<HashMap<ObjectId, GlobalStatePathMetaItem>>>,
}

impl GlobalStatePathMetaManager {
    pub fn new(
        isotate: &str,
        root_state: GlobalStateOutputProcessorRef,
        root_state_service: GlobalStateLocalService,
        category: GlobalStateCategory,
        noc: NamedObjectCacheRef,
        device_id: DeviceId,
    ) -> Self {
        Self {
            isolate: isotate.to_owned(),
            root_state,
            root_state_service,
            category,
            noc,
            device_id,
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
            self.device_id.clone(),
        );

        Arc::new(raw)
    }

    pub fn clone_processor(&self) -> GlobalStateMetaInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    fn get_dec_meta(
        &self,
        dec_id: &ObjectId,
        auto_create: bool,
    ) -> Option<GlobalStateDecPathMetaManagerRef> {
        let mut list = self.all.lock().unwrap();
        match list.entry(dec_id.to_owned()) {
            Entry::Occupied(mut o) => {
                let item = o.get_mut();
                item.last_access = bucky_time_now();
                Some(item.manager.clone())
            }
            Entry::Vacant(v) => {
                if !auto_create && !self.root_state_service.state().is_dec_exists(dec_id) {
                    return None;
                }

                let manager = self.new_dec_meta(v.key().to_owned());
                let item = GlobalStatePathMetaItem {
                    manager: manager.clone(),
                    last_access: bucky_time_now(),
                };

                v.insert(item);
                Some(manager)
            }
        }
    }

    pub async fn get_global_state_meta(
        &self,
        dec_id: &ObjectId,
        auto_create: bool,
    ) -> BuckyResult<GlobalStatePathMetaSyncCollection> {
        let ret = self
            .get_option_global_state_meta(dec_id, auto_create)
            .await?;
        if ret.is_none() {
            let msg = format!(
                "global state path meta for dec not found! {}, dec={:?}",
                self.root_state.get_category(),
                dec_id
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        Ok(ret.unwrap())
    }

    pub async fn get_option_global_state_meta(
        &self,
        dec_id: &ObjectId,
        auto_create: bool,
    ) -> BuckyResult<Option<GlobalStatePathMetaSyncCollection>> {
        let ret = self.get_dec_meta(dec_id, auto_create);
        if ret.is_none() {
            return Ok(None);
        }

        let manager = ret.unwrap();
        manager.get_global_state_meta().await.map(|v| Some(v))
    }

    fn get_dec_id(common: &MetaInputRequestCommon) -> &ObjectId {
        if let Some(dec_id) = &common.target_dec_id {
            dec_id
        } else {
            &common.source.dec
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateMetaInputProcessor for GlobalStatePathMetaManager {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessInputResponse> {
        let meta = self
            .get_global_state_meta(Self::get_dec_id(&req.common), true)
            .await?;
        let updated = meta.add_access(req.item).await?;

        let resp = GlobalStateMetaAddAccessInputResponse { updated };
        Ok(resp)
    }

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessInputResponse> {
        let ret = self
            .get_option_global_state_meta(Self::get_dec_id(&req.common), false)
            .await?;
        if ret.is_none() {
            let resp = GlobalStateMetaRemoveAccessInputResponse { item: None };

            return Ok(resp);
        }

        let meta = ret.unwrap();
        let item = meta.remove_access(req.item).await?;

        let resp = GlobalStateMetaRemoveAccessInputResponse { item };

        Ok(resp)
    }

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessInputResponse> {
        let ret = self
            .get_option_global_state_meta(Self::get_dec_id(&req.common), false)
            .await?;
        if ret.is_none() {
            let resp = GlobalStateMetaClearAccessInputResponse { count: 0 };
            return Ok(resp);
        }

        let meta = ret.unwrap();
        let count = meta.clear_access().await? as u32;

        let resp = GlobalStateMetaClearAccessInputResponse { count };
        Ok(resp)
    }

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkInputResponse> {
        let meta = self
            .get_global_state_meta(Self::get_dec_id(&req.common), true)
            .await?;
        let updated = meta.add_link(req.source, req.target).await?;

        let resp = GlobalStateMetaAddLinkInputResponse { updated };
        Ok(resp)
    }

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkInputResponse> {
        let ret = self
            .get_option_global_state_meta(Self::get_dec_id(&req.common), false)
            .await?;
        if ret.is_none() {
            let resp = GlobalStateMetaRemoveLinkInputResponse { item: None };

            return Ok(resp);
        }

        let meta = ret.unwrap();
        let item = meta.remove_link(&req.source).await?;

        let resp = GlobalStateMetaRemoveLinkInputResponse { item };

        Ok(resp)
    }

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkInputResponse> {
        let ret = self
            .get_option_global_state_meta(Self::get_dec_id(&req.common), false)
            .await?;
        if ret.is_none() {
            let resp = GlobalStateMetaClearLinkInputResponse { count: 0 };
            return Ok(resp);
        }

        let meta = ret.unwrap();
        let count = meta.clear_link().await? as u32;

        let resp = GlobalStateMetaClearLinkInputResponse { count };
        Ok(resp)
    }

    async fn add_object_meta(
        &self,
        req: GlobalStateMetaAddObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddObjectMetaInputResponse> {
        let meta = self
            .get_global_state_meta(Self::get_dec_id(&req.common), true)
            .await?;
        let updated = meta.add_object_meta(req.item).await?;

        let resp = GlobalStateMetaAddObjectMetaInputResponse { updated };
        Ok(resp)
    }

    async fn remove_object_meta(
        &self,
        req: GlobalStateMetaRemoveObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveObjectMetaInputResponse> {
        let ret = self
            .get_option_global_state_meta(Self::get_dec_id(&req.common), false)
            .await?;
        if ret.is_none() {
            let resp = GlobalStateMetaRemoveObjectMetaInputResponse { item: None };

            return Ok(resp);
        }

        let meta = ret.unwrap();
        let item = meta.remove_object_meta(req.item).await?;

        let resp = GlobalStateMetaRemoveObjectMetaInputResponse { item };

        Ok(resp)
    }

    async fn clear_object_meta(
        &self,
        req: GlobalStateMetaClearObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearObjectMetaInputResponse> {
        let ret = self
            .get_option_global_state_meta(Self::get_dec_id(&req.common), false)
            .await?;
        if ret.is_none() {
            let resp = GlobalStateMetaClearObjectMetaInputResponse { count: 0 };
            return Ok(resp);
        }

        let meta = ret.unwrap();
        let count = meta.clear_object_meta().await? as u32;

        let resp = GlobalStateMetaClearObjectMetaInputResponse { count };
        Ok(resp)
    }

    // path config
    async fn add_path_config(
        &self,
        req: GlobalStateMetaAddPathConfigInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddPathConfigInputResponse> {
        let meta = self
            .get_global_state_meta(Self::get_dec_id(&req.common), true)
            .await?;
        let updated = meta.add_path_config(req.item).await?;

        let resp = GlobalStateMetaAddPathConfigInputResponse { updated };
        Ok(resp)
    }

    async fn remove_path_config(
        &self,
        req: GlobalStateMetaRemovePathConfigInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemovePathConfigInputResponse> {
        let ret = self
            .get_option_global_state_meta(Self::get_dec_id(&req.common), false)
            .await?;
        if ret.is_none() {
            let resp = GlobalStateMetaRemovePathConfigInputResponse { item: None };

            return Ok(resp);
        }

        let meta = ret.unwrap();
        let item = meta.remove_path_config(req.item).await?;

        let resp = GlobalStateMetaRemovePathConfigInputResponse { item };

        Ok(resp)
    }

    async fn clear_path_config(
        &self,
        req: GlobalStateMetaClearPathConfigInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearPathConfigInputResponse> {
        let ret = self
            .get_option_global_state_meta(Self::get_dec_id(&req.common), false)
            .await?;
        if ret.is_none() {
            let resp = GlobalStateMetaClearPathConfigInputResponse { count: 0 };
            return Ok(resp);
        }

        let meta = ret.unwrap();
        let count = meta.clear_path_config().await? as u32;

        let resp = GlobalStateMetaClearPathConfigInputResponse { count };
        Ok(resp)
    }
}

pub type GlobalStatePathMetaManagerRef = Arc<GlobalStatePathMetaManager>;
