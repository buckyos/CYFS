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

    // object meta
    async fn add_object_meta(
        &self,
        req: GlobalStateMetaAddObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddObjectMetaInputResponse> {
        self.check_access("global_state.meta.add_object_meta", &req.common)?;

        self.next.add_object_meta(req).await
    }

    async fn remove_object_meta(
        &self,
        req: GlobalStateMetaRemoveObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveObjectMetaInputResponse> {
        self.check_access("global_state.meta.remove_object_meta", &req.common)?;

        self.next.remove_object_meta(req).await
    }

    async fn clear_object_meta(
        &self,
        req: GlobalStateMetaClearObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearObjectMetaInputResponse> {
        self.check_access("global_state.meta.clear_object_meta", &req.common)?;

        self.next.clear_object_meta(req).await
    }

    // path config
    async fn add_path_config(
        &self,
        req: GlobalStateMetaAddPathConfigInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddPathConfigInputResponse> {
        self.check_access("global_state.meta.add_path_config", &req.common)?;

        self.next.add_path_config(req).await
    }

    async fn remove_path_config(
        &self,
        req: GlobalStateMetaRemovePathConfigInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemovePathConfigInputResponse> {
        self.check_access("global_state.meta.remove_path_config", &req.common)?;

        self.next.remove_path_config(req).await
    }

    async fn clear_path_config(
        &self,
        req: GlobalStateMetaClearPathConfigInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearPathConfigInputResponse> {
        self.check_access("global_state.meta.clear_path_config", &req.common)?;

        self.next.clear_path_config(req).await
    }
}
