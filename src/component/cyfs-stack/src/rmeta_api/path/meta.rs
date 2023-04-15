use super::super::object::*;
use super::access::*;
use super::config::*;
use super::link::*;
use super::storage::*;
use cyfs_base::*;
use cyfs_lib::*;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStatePathMeta {
    access: GlobalStatePathAccessList,
    link: GlobalStatePathLinkList,
    config: GlobalStatePathConfigList,

    #[serde(default)]
    object: GlobalStateObjectMetaList,
}

impl Default for GlobalStatePathMeta {
    fn default() -> Self {
        Self {
            access: GlobalStatePathAccessList::default(),
            link: GlobalStatePathLinkList::default(),
            config: GlobalStatePathConfigList::default(),
            object: GlobalStateObjectMetaList::default(),
        }
    }
}

declare_collection_codec_for_serde!(GlobalStatePathMeta);

#[derive(Clone)]
pub struct GlobalStatePathMetaSyncCollection {
    // current device id
    device_id: DeviceId,

    meta: Arc<NOCCollectionRWAsync<GlobalStatePathMeta>>,

    // dump to local file for debug and review
    storage: Arc<GlobalStatePathMetaStorage>,
}

impl GlobalStatePathMetaSyncCollection {
    pub fn new(
        device_id: DeviceId,
        storage: Arc<GlobalStatePathMetaStorage>,
        meta: NOCCollectionRWAsync<GlobalStatePathMeta>,
    ) -> Self {
        Self {
            device_id,
            meta: Arc::new(meta),
            storage,
        }
    }

    pub fn into_processor(self) -> GlobalStateMetaRawProcessorRef {
        Arc::new(Box::new(self))
    }

    async fn dump(&self) {
        let data = {
            let meta = self.meta.coll().read().await;
            serde_json::to_string(&meta as &GlobalStatePathMeta).unwrap()
        };

        let storage = self.storage.clone();
        async_std::task::spawn(async move { storage.save(data).await });
    }

    pub async fn add_access(&self, item: GlobalStatePathAccessItem) -> BuckyResult<bool> {
        if !item.check_valid() {
            let msg = format!("invalid access item! {}", item);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        {
            let mut meta = self.meta.coll().write().await;
            let ret = meta.access.add(item);
            if !ret {
                return Ok(false);
            }
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(true)
    }

    pub async fn remove_access(
        &self,
        item: GlobalStatePathAccessItem,
    ) -> BuckyResult<Option<GlobalStatePathAccessItem>> {
        let ret = {
            let mut meta = self.meta.coll().write().await;
            meta.access.remove(item)
        };

        if ret.is_none() {
            return Ok(None);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(ret)
    }

    pub async fn clear_access(&self) -> BuckyResult<usize> {
        let ret = {
            let mut meta = self.meta.coll().write().await;
            meta.access.clear()
        };

        if ret == 0 {
            return Ok(ret);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(ret)
    }

    pub async fn check_access<'d, 'a, 'b>(
        &self,
        req: GlobalStateAccessRequest<'d, 'a, 'b>,
        handler: &GlobalStatePathHandlerRef,
    ) -> BuckyResult<()> {
        let meta = self.meta.coll().read().await;
        meta.access.check(req, &self.device_id, handler).await
    }

    pub async fn add_link(
        &self,
        source: impl Into<String> + AsRef<str>,
        target: impl Into<String> + AsRef<str>,
    ) -> BuckyResult<bool> {
        {
            let mut meta = self.meta.coll().write().await;
            let ret = meta.link.add(source, target)?;
            if !ret {
                return Ok(false);
            }
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(true)
    }

    pub async fn remove_link(&self, source: &str) -> BuckyResult<Option<GlobalStatePathLinkItem>> {
        let ret = {
            let mut meta = self.meta.coll().write().await;
            meta.link.remove(source)?
        };

        if ret.is_none() {
            return Ok(None);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(ret)
    }

    pub async fn clear_link(&self) -> BuckyResult<usize> {
        let ret = {
            let mut meta = self.meta.coll().write().await;
            meta.link.clear()
        };

        if ret == 0 {
            return Ok(ret);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(ret)
    }

    pub async fn resolve_link(&self, source: &str) -> BuckyResult<Option<String>> {
        let meta = self.meta.coll().read().await;
        meta.link.resolve(source)
    }

    // object meta
    pub async fn add_object_meta(&self, item: GlobalStateObjectMetaItem) -> BuckyResult<bool> {
        let item = ObjectMeta::new(item)?;
        if !item.check_valid() {
            let msg = format!("invalid object meta item! {}", item);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        {
            let mut meta = self.meta.coll().write().await;
            let ret = meta.object.add(item);
            if !ret {
                return Ok(false);
            }
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(true)
    }

    pub async fn remove_object_meta(
        &self,
        item: GlobalStateObjectMetaItem,
    ) -> BuckyResult<Option<GlobalStateObjectMetaItem>> {
        let item = ObjectMeta::new_uninit(item);

        let ret = {
            let mut meta = self.meta.coll().write().await;
            meta.object.remove(&item)
        };

        if ret.is_none() {
            return Ok(None);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        let item = ret.unwrap();
        let ret = GlobalStateObjectMetaItem {
            selector: item.selector.into_exp(),
            access: item.access,
            depth: item.depth,
        };

        Ok(Some(ret))
    }

    pub async fn clear_object_meta(&self) -> BuckyResult<usize> {
        let ret = {
            let mut meta = self.meta.coll().write().await;
            meta.object.clear()
        };

        if ret == 0 {
            return Ok(ret);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(ret)
    }

    pub async fn check_object_access(
        &self,
        target_dec_id: &ObjectId,
        object_data: &dyn ObjectSelectorDataProvider,
        source: &RequestSourceInfo,
        permissions: AccessPermissions,
    ) -> BuckyResult<Option<()>> {
        let meta = self.meta.coll().read().await;
        meta.object.check(
            target_dec_id,
            object_data,
            source,
            permissions,
            &self.device_id,
        )
    }

    pub async fn query_object_meta(
        &self,
        object_data: &dyn ObjectSelectorDataProvider,
    ) -> Option<GlobalStateObjectMetaConfigItemValue> {
        let meta = self.meta.coll().read().await;
        meta.object
            .query_object_meta(object_data)
            .map(|ret| GlobalStateObjectMetaConfigItemValue {
                access: ret.access.clone(),
                depth: ret.depth,
            })
    }

    // path config
    pub async fn add_path_config(&self, item: GlobalStatePathConfigItem) -> BuckyResult<bool> {
        {
            let mut meta = self.meta.coll().write().await;
            let ret = meta.config.add(item);
            if !ret {
                return Ok(false);
            }
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(true)
    }

    pub async fn remove_path_config(
        &self,
        item: GlobalStatePathConfigItem,
    ) -> BuckyResult<Option<GlobalStatePathConfigItem>> {
        let ret = {
            let mut meta = self.meta.coll().write().await;
            meta.config.remove(item)
        };

        if ret.is_none() {
            return Ok(None);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(ret)
    }

    pub async fn clear_path_config(&self) -> BuckyResult<usize> {
        let ret = {
            let mut meta = self.meta.coll().write().await;
            meta.config.clear()
        };

        if ret == 0 {
            return Ok(ret);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump().await;

        Ok(ret)
    }

    pub async fn query_path_config(&self, path: &str) -> Option<GlobalStatePathConfigItemValue> {
        let ret = {
            let meta = self.meta.coll().write().await;
            match meta.config.query(path) {
                Some(item) => Some(GlobalStatePathConfigItemValue {
                    storage_state: item.storage_state,
                    depth: item.depth,
                }),
                None => None,
            }
        };

        ret
    }
}

#[async_trait::async_trait]
impl GlobalStateMetaRawProcessor for GlobalStatePathMetaSyncCollection {
    async fn add_access(&self, item: GlobalStatePathAccessItem) -> BuckyResult<bool> {
        Self::add_access(&self, item).await
    }

    async fn remove_access(
        &self,
        item: GlobalStatePathAccessItem,
    ) -> BuckyResult<Option<GlobalStatePathAccessItem>> {
        Self::remove_access(self, item).await
    }

    async fn clear_access(&self) -> BuckyResult<usize> {
        Self::clear_access(self).await
    }

    async fn check_access<'d, 'a, 'b>(
        &self,
        req: GlobalStateAccessRequest<'d, 'a, 'b>,
        handler: &GlobalStatePathHandlerRef,
    ) -> BuckyResult<()> {
        Self::check_access(self, req, handler).await
    }

    // link relate methods
    async fn add_link(&self, source: &str, target: &str) -> BuckyResult<bool> {
        Self::add_link(self, source, target).await
    }

    async fn remove_link(&self, source: &str) -> BuckyResult<Option<GlobalStatePathLinkItem>> {
        Self::remove_link(self, source).await
    }

    async fn clear_link(&self) -> BuckyResult<usize> {
        Self::clear_link(self).await
    }

    async fn resolve_link(&self, source: &str) -> BuckyResult<Option<String>> {
        Self::resolve_link(self, source).await
    }

    // object meta
    async fn add_object_meta(&self, item: GlobalStateObjectMetaItem) -> BuckyResult<bool> {
        Self::add_object_meta(self, item).await
    }

    async fn remove_object_meta(
        &self,
        item: GlobalStateObjectMetaItem,
    ) -> BuckyResult<Option<GlobalStateObjectMetaItem>> {
        Self::remove_object_meta(self, item).await
    }

    async fn clear_object_meta(&self) -> BuckyResult<usize> {
        Self::clear_object_meta(self).await
    }

    async fn query_object_meta(
        &self,
        object_data: &dyn ObjectSelectorDataProvider,
    ) -> Option<GlobalStateObjectMetaConfigItemValue> {
        Self::query_object_meta(self, object_data).await
    }
    
    async fn check_object_access(
        &self,
        target_dec_id: &ObjectId,
        object_data: &dyn ObjectSelectorDataProvider,
        source: &RequestSourceInfo,
        permissions: AccessPermissions,
    ) -> BuckyResult<Option<()>> {
        Self::check_object_access(self, target_dec_id, object_data, source, permissions).await
    }

    // path config
    async fn add_path_config(&self, item: GlobalStatePathConfigItem) -> BuckyResult<bool> {
        Self::add_path_config(self, item).await
    }

    async fn remove_path_config(
        &self,
        item: GlobalStatePathConfigItem,
    ) -> BuckyResult<Option<GlobalStatePathConfigItem>> {
        Self::remove_path_config(self, item).await
    }
    async fn clear_path_config(&self) -> BuckyResult<usize> {
        Self::clear_path_config(self).await
    }

    async fn query_path_config(&self, path: &str) -> Option<GlobalStatePathConfigItemValue> {
        Self::query_path_config(self, path).await
    }
}
