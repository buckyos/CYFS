use super::def::*;
use super::output_request::*;
use crate::{RequestSourceInfo, GlobalStateCategory};
use cyfs_base::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait GlobalStateMetaOutputProcessor: Sync + Send + 'static {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessOutputResponse>;

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessOutputResponse>;

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessOutputResponse>;

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkOutputResponse>;

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkOutputResponse>;

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkOutputResponse>;

    // object meta
    async fn add_object_meta(
        &self,
        req: GlobalStateMetaAddObjectMetaOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddObjectMetaOutputResponse>;

    async fn remove_object_meta(
        &self,
        req: GlobalStateMetaRemoveObjectMetaOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveObjectMetaOutputResponse>;

    async fn clear_object_meta(
        &self,
        req: GlobalStateMetaClearObjectMetaOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearObjectMetaOutputResponse>;

    // path config
    async fn add_path_config(
        &self,
        req: GlobalStateMetaAddPathConfigOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddPathConfigOutputResponse>;

    async fn remove_path_config(
        &self,
        req: GlobalStateMetaRemovePathConfigOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemovePathConfigOutputResponse>;

    async fn clear_path_config(
        &self,
        req: GlobalStateMetaClearPathConfigOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearPathConfigOutputResponse>;
}

pub type GlobalStateMetaOutputProcessorRef = Arc<Box<dyn GlobalStateMetaOutputProcessor>>;

#[async_trait::async_trait]
pub trait GlobalStateMetaRawProcessor: Send + Sync {
    // access relate methods
    async fn add_access(&self, item: GlobalStatePathAccessItem) -> BuckyResult<bool>;

    async fn remove_access(
        &self,
        item: GlobalStatePathAccessItem,
    ) -> BuckyResult<Option<GlobalStatePathAccessItem>>;

    async fn clear_access(&self) -> BuckyResult<usize>;

    async fn check_access<'d, 'a, 'b>(
        &self,
        req: GlobalStateAccessRequest<'d, 'a, 'b>,
        handler: &GlobalStatePathHandlerRef,
    ) -> BuckyResult<()>;

    // link relate methods
    async fn add_link(
        &self,
        source: &str,
        target: &str,
    ) -> BuckyResult<bool>;

    async fn remove_link(&self, source: &str) -> BuckyResult<Option<GlobalStatePathLinkItem>>;

    async fn clear_link(&self) -> BuckyResult<usize>;

    async fn resolve_link(&self, source: &str) -> BuckyResult<Option<String>>;

    // object meta
    async fn add_object_meta(&self, item: GlobalStateObjectMetaItem) -> BuckyResult<bool>;

    async fn remove_object_meta(
        &self,
        item: GlobalStateObjectMetaItem,
    ) -> BuckyResult<Option<GlobalStateObjectMetaItem>>;

    async fn clear_object_meta(&self) -> BuckyResult<usize>;

    async fn check_object_access(
        &self,
        target_dec_id: &ObjectId,
        object_data: &dyn ObjectSelectorDataProvider,
        source: &RequestSourceInfo,
        permissions: AccessPermissions,
    ) -> BuckyResult<Option<()>>;

    async fn query_object_meta(
        &self,
        object_data: &dyn ObjectSelectorDataProvider,
    ) -> Option<GlobalStateObjectMetaConfigItemValue>;

    // path config
    async fn add_path_config(&self, item: GlobalStatePathConfigItem) -> BuckyResult<bool>;

    async fn remove_path_config(
        &self,
        item: GlobalStatePathConfigItem,
    ) -> BuckyResult<Option<GlobalStatePathConfigItem>>;
    async fn clear_path_config(&self) -> BuckyResult<usize>;

    async fn query_path_config(&self, path: &str) -> Option<GlobalStatePathConfigItemValue>;
}

pub type GlobalStateMetaRawProcessorRef = Arc<Box<dyn GlobalStateMetaRawProcessor>>;


#[async_trait::async_trait]
pub trait GlobalStateMetaManagerRawProcessor: Send + Sync {
    async fn get_global_state_meta(
        &self,
        dec_id: &ObjectId,
        category: GlobalStateCategory,
        auto_create: bool,
    ) -> BuckyResult<Option<GlobalStateMetaRawProcessorRef>>;
}

pub type GlobalStateMetaManagerRawProcessorRef = Arc<Box<dyn GlobalStateMetaManagerRawProcessor>>;