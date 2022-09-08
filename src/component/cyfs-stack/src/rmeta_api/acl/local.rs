use crate::acl::*;
use crate::rmeta::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 限定在同zone内操作
pub(crate) struct GlobalStateMetaAclInnerInputProcessor {
    acl: AclManagerRef,
    next: GlobalStateMetaInputProcessorRef,
}

impl GlobalStateMetaAclInnerInputProcessor {
    pub(crate) fn new(
        acl: AclManagerRef,
        next: GlobalStateMetaInputProcessorRef,
    ) -> GlobalStateMetaInputProcessorRef {
        let ret = Self { acl, next };

        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl GlobalStateMetaInputProcessor for GlobalStateMetaAclInnerInputProcessor {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessInputResponse> {
        self.acl
            .check_local_zone_permit("global_state.meta.add_access", &req.common.source)
            .await?;

        self.next.add_access(req).await
    }

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessInputResponse> {
        self.acl
            .check_local_zone_permit("global_state.meta.remove_access", &req.common.source)
            .await?;

        self.next.remove_access(req).await
    }

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessInputResponse> {
        self.acl
            .check_local_zone_permit("global_state.meta.clear_access", &req.common.source)
            .await?;

        self.next.clear_access(req).await
    }

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkInputResponse> {
        self.acl
            .check_local_zone_permit("global_state.meta.add_link", &req.common.source)
            .await?;

        self.next.add_link(req).await
    }

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkInputResponse> {
        self.acl
            .check_local_zone_permit("global_state.meta.remove_link", &req.common.source)
            .await?;

        self.next.remove_link(req).await
    }

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkInputResponse> {
        self.acl
            .check_local_zone_permit("global_state.meta.clear_link", &req.common.source)
            .await?;

        self.next.clear_link(req).await
    }
}
