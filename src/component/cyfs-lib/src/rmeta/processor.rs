use super::output_request::*;
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