use crate::rmeta::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 限定在同zone内操作
pub(crate) struct GlobalStateMetaAclInnerInputProcessor {
    next: GlobalStateMetaInputProcessorRef,
}

impl GlobalStateMetaAclInnerInputProcessor {
    pub(crate) fn new(next: GlobalStateMetaInputProcessorRef) -> GlobalStateMetaInputProcessorRef {
        let ret = Self { next };

        Arc::new(Box::new(ret))
    }

    fn check_access(&self, service: &str, common: &MetaInputRequestCommon) -> BuckyResult<()> {
        common.source.check_current_zone(service)?;

        if common
            .source
            .check_target_dec_permission(&common.target_dec_id)
        {
            return Ok(());
        }

        let msg = format!(
            "global state meta access can't be used between different dec! {}, source={}, target={:?}",
            service, common.source, common.target_dec_id,
        );
        error!("{}", msg);

        Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
    }
}

#[async_trait::async_trait]
impl GlobalStateMetaInputProcessor for GlobalStateMetaAclInnerInputProcessor {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessInputResponse> {
        self.check_access("global_state.meta.add_access", &req.common)?;

        self.next.add_access(req).await
    }

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessInputResponse> {
        self.check_access("global_state.meta.remove_access", &req.common)?;

        self.next.remove_access(req).await
    }

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessInputResponse> {
        self.check_access("global_state.meta.clear_access", &req.common)?;

        self.next.clear_access(req).await
    }

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkInputResponse> {
        self.check_access("global_state.meta.add_link", &req.common)?;

        self.next.add_link(req).await
    }

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkInputResponse> {
        self.check_access("global_state.meta.remove_link", &req.common)?;

        self.next.remove_link(req).await
    }

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkInputResponse> {
        self.check_access("global_state.meta.clear_link", &req.common)?;

        self.next.clear_link(req).await
    }
}
