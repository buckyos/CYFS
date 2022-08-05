use crate::acl::*;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 限定在同zone内操作
pub(crate) struct GlobalStateAclInnerInputProcessor {
    acl: AclManagerRef,
    next: GlobalStateInputProcessorRef,
}

impl GlobalStateAclInnerInputProcessor {
    pub(crate) fn new(
        acl: AclManagerRef,
        next: GlobalStateInputProcessorRef,
    ) -> GlobalStateInputProcessorRef {
        let ret = Self { acl, next };

        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl GlobalStateInputProcessor for GlobalStateAclInnerInputProcessor {
    fn create_op_env_processor(&self) -> OpEnvInputProcessorRef {
        let processor = self.next.create_op_env_processor();
        OpEnvAclInnerInputProcessor::new(self.acl.clone(), processor)
    }

    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootInputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootInputResponse> {
        self.acl
            .check_local_zone_permit("global_state.get_root", &req.common.source)
            .await?;

        self.next.get_current_root(req).await
    }

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvInputRequest,
    ) -> BuckyResult<RootStateCreateOpEnvInputResponse> {
        self.acl
            .check_local_zone_permit("global_state.create_op_env", &req.common.source)
            .await?;

        self.next.create_op_env(req).await
    }
}

pub(crate) struct OpEnvAclInnerInputProcessor {
    acl: AclManagerRef,
    next: OpEnvInputProcessorRef,
}

impl OpEnvAclInnerInputProcessor {
    pub(crate) fn new(acl: AclManagerRef, next: OpEnvInputProcessorRef) -> OpEnvInputProcessorRef {
        let ret = Self { acl, next };

        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl OpEnvInputProcessor for OpEnvAclInnerInputProcessor {
    // single_op_env methods
    async fn load(&self, req: OpEnvLoadInputRequest) -> BuckyResult<()> {
        self.acl
            .check_local_zone_permit("op_env.load", &req.common.source)
            .await?;

        self.next.load(req).await
    }

    async fn load_by_path(&self, req: OpEnvLoadByPathInputRequest) -> BuckyResult<()> {
        self.acl
            .check_local_zone_permit("op_env.load_by_path", &req.common.source)
            .await?;

        self.next.load_by_path(req).await
    }

    async fn create_new(&self, req: OpEnvCreateNewInputRequest) -> BuckyResult<()> {
        self.acl
            .check_local_zone_permit("op_env.create_new", &req.common.source)
            .await?;

        self.next.create_new(req).await
    }

    // lock
    async fn lock(&self, req: OpEnvLockInputRequest) -> BuckyResult<()> {
        self.acl
            .check_local_zone_permit("op_env.lock", &req.common.source)
            .await?;

        self.next.lock(req).await
    }

    // get_current_root
    async fn get_current_root(
        &self,
        req: OpEnvGetCurrentRootInputRequest,
    ) -> BuckyResult<OpEnvGetCurrentRootInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.get_current_root", &req.common.source)
            .await?;

        self.next.get_current_root(req).await
    }

    // transcation
    async fn commit(&self, req: OpEnvCommitInputRequest) -> BuckyResult<OpEnvCommitInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.commit", &req.common.source)
            .await?;

        self.next.commit(req).await
    }

    async fn abort(&self, req: OpEnvAbortInputRequest) -> BuckyResult<()> {
        self.acl
            .check_local_zone_permit("op_env.abort", &req.common.source)
            .await?;

        self.next.abort(req).await
    }

    // map methods
    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyInputRequest,
    ) -> BuckyResult<OpEnvGetByKeyInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.get_by_key", &req.common.source)
            .await?;

        self.next.get_by_key(req).await
    }

    async fn insert_with_key(&self, req: OpEnvInsertWithKeyInputRequest) -> BuckyResult<()> {
        self.acl
            .check_local_zone_permit("op_env.insert_with_key", &req.common.source)
            .await?;

        self.next.insert_with_key(req).await
    }

    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyInputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.set_with_key", &req.common.source)
            .await?;

        self.next.set_with_key(req).await
    }

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyInputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.remove_with_key", &req.common.source)
            .await?;

        self.next.remove_with_key(req).await
    }

    // set methods
    async fn contains(
        &self,
        req: OpEnvContainsInputRequest,
    ) -> BuckyResult<OpEnvContainsInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.contains", &req.common.source)
            .await?;

        self.next.contains(req).await
    }

    async fn insert(&self, req: OpEnvInsertInputRequest) -> BuckyResult<OpEnvInsertInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.insert", &req.common.source)
            .await?;

        self.next.insert(req).await
    }

    async fn remove(&self, req: OpEnvRemoveInputRequest) -> BuckyResult<OpEnvRemoveInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.remove", &req.common.source)
            .await?;

        self.next.remove(req).await
    }

    // iterator methods
    async fn next(&self, req: OpEnvNextInputRequest) -> BuckyResult<OpEnvNextInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.next", &req.common.source)
            .await?;

        self.next.next(req).await
    }

    async fn reset(&self, req: OpEnvResetInputRequest) -> BuckyResult<()> {
        self.acl
            .check_local_zone_permit("op_env.reset", &req.common.source)
            .await?;

        self.next.reset(req).await
    }

    async fn list(&self, req: OpEnvListInputRequest) -> BuckyResult<OpEnvListInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.list", &req.common.source)
            .await?;

        self.next.list(req).await
    }

    // metadata
    async fn metadata(
        &self,
        req: OpEnvMetadataInputRequest,
    ) -> BuckyResult<OpEnvMetadataInputResponse> {
        self.acl
            .check_local_zone_permit("op_env.metadata", &req.common.source)
            .await?;

        self.next.metadata(req).await
    }
}
