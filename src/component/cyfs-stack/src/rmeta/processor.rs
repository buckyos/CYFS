use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait GlobalStateMetaInputProcessor: Sync + Send + 'static {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessInputResponse>;

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessInputResponse>;

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessInputResponse>;

    // link
    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkInputResponse>;

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkInputResponse>;

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkInputResponse>;

    // object meta
    async fn add_object_meta(
        &self,
        req: GlobalStateMetaAddObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddObjectMetaInputResponse>;

    async fn remove_object_meta(
        &self,
        req: GlobalStateMetaRemoveObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveObjectMetaInputResponse>;

    async fn clear_object_meta(
        &self,
        req: GlobalStateMetaClearObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearObjectMetaInputResponse>;

    // path config
    async fn add_path_config(
        &self,
        req: GlobalStateMetaAddPathConfigInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddPathConfigInputResponse>;

    async fn remove_path_config(
        &self,
        req: GlobalStateMetaRemovePathConfigInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemovePathConfigInputResponse>;

    async fn clear_path_config(
        &self,
        req: GlobalStateMetaClearPathConfigInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearPathConfigInputResponse>;
}

pub type GlobalStateMetaInputProcessorRef = Arc<Box<dyn GlobalStateMetaInputProcessor>>;
