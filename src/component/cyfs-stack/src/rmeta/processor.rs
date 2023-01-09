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
}

pub type GlobalStateMetaInputProcessorRef = Arc<Box<dyn GlobalStateMetaInputProcessor>>;
