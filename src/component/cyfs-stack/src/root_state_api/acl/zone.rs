use crate::acl::AclManagerRef;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 限定在同zone内操作
pub(crate) struct GlobalStateAclZoneInputProcessor {
    acl: AclManagerRef,
    next: GlobalStateInputProcessorRef,
}

impl GlobalStateAclZoneInputProcessor {
    pub(crate) fn new(
        acl: AclManagerRef,
        next: GlobalStateInputProcessorRef,
    ) -> GlobalStateInputProcessorRef {
        let ret = Self { acl, next };

        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl GlobalStateInputProcessor for GlobalStateAclZoneInputProcessor {
    fn create_op_env_processor(&self) -> OpEnvInputProcessorRef {
        let processor = self.next.create_op_env_processor();
        OpEnvAclInnerInputProcessor::new(self.acl.clone(), processor)
    }

    fn get_category(&self) -> GlobalStateCategory {
        self.next.get_category()
    }

    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootInputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootInputResponse> {
        req.common
            .source
            .check_current_zone("global_state.get_root")?;

        if !req
            .common
            .source
            .check_target_dec_permission(&req.common.target_dec_id)
        {
            let global_state = match req.root_type {
                RootStateRootType::Global => {
                    RequestGlobalStatePath {
                        global_state_category: Some(self.get_category()),
                        global_state_root: None,
                        dec_id: Some(cyfs_core::get_system_dec_app().to_owned()),
                        req_path: Some(CYFS_GLOBAL_STATE_ROOT_VIRTUAL_PATH.to_owned()),
                        req_query_string: None,
                    }
                }
                RootStateRootType::Dec => {
                    RequestGlobalStatePath {
                        global_state_category: Some(self.get_category()),
                        global_state_root: None,
                        dec_id: req.common.target_dec_id.clone(),
                        req_path: None, // None will treat as /
                        req_query_string: None,
                    }
                }
            };
            
            self.acl
                .global_state_meta()
                .check_access(&req.common.source, &global_state, RequestOpType::Read)
                .await?;
        }

        self.next.get_current_root(req).await
    }

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvInputRequest,
    ) -> BuckyResult<RootStateCreateOpEnvInputResponse> {
        req.common
            .source
            .check_current_zone("global_state.create_op_env")?;

        if !req
            .common
            .source
            .check_target_dec_permission(&req.common.target_dec_id)
        {
            if req.access.is_none() {
                let msg = format!(
                    "op_env between different dec should specified the access param! source={}, target={:?}",
                    req.common.source, req.common.target_dec_id,
                );
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
            }

            let access = req.access.as_ref().unwrap();

            let (req_path, req_query_string) = RequestGlobalStatePath::parse_req_path_with_query_string_owned(&access.path);
            let global_state = RequestGlobalStatePath {
                global_state_category: Some(self.get_category()),
                global_state_root: None,
                dec_id: req.common.target_dec_id.clone(),
                req_path: Some(req_path),
                req_query_string,
            };

            self.acl
                .global_state_meta()
                .check_access(&req.common.source, &global_state, access.access)
                .await?;
        }

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

    fn check_access(&self, service: &str, common: &OpEnvInputRequestCommon) -> BuckyResult<()> {
        common.source.check_current_zone(service)?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl OpEnvInputProcessor for OpEnvAclInnerInputProcessor {
    fn get_category(&self) -> GlobalStateCategory {
        self.next.get_category()
    }

    // single_op_env methods
    async fn load(&self, req: OpEnvLoadInputRequest) -> BuckyResult<()> {
        self.check_access("op_env.load", &req.common)?;

        self.next.load(req).await
    }

    async fn load_by_path(&self, req: OpEnvLoadByPathInputRequest) -> BuckyResult<()> {
        self.check_access("op_env.load_by_path", &req.common)?;

        // 如果是跨dec加载path，那么需要额外的rmeta校验权限
        if !req
            .common
            .source
            .check_target_dec_permission(&req.common.target_dec_id)
        {
            let (req_path, req_query_string) = RequestGlobalStatePath::parse_req_path_with_query_string_owned(&req.path);
            let global_state = RequestGlobalStatePath {
                global_state_category: Some(self.next.get_category()),
                global_state_root: None,
                dec_id: req.common.target_dec_id.clone(),
                req_path: Some(req_path),
                req_query_string,
            };

            self.acl
                .global_state_meta()
                .check_access(&req.common.source, &global_state, RequestOpType::Read)
                .await?;
        }

        self.next.load_by_path(req).await
    }

    async fn create_new(&self, req: OpEnvCreateNewInputRequest) -> BuckyResult<()> {
        self.check_access("op_env.create_new", &req.common)?;

        self.next.create_new(req).await
    }

    // lock
    async fn lock(&self, req: OpEnvLockInputRequest) -> BuckyResult<()> {
        self.check_access("op_env.lock", &req.common)?;

        self.next.lock(req).await
    }

    // get_current_root
    async fn get_current_root(
        &self,
        req: OpEnvGetCurrentRootInputRequest,
    ) -> BuckyResult<OpEnvGetCurrentRootInputResponse> {
        self.check_access("op_env.get_current_root", &req.common)?;

        self.next.get_current_root(req).await
    }

    // transcation
    async fn commit(&self, req: OpEnvCommitInputRequest) -> BuckyResult<OpEnvCommitInputResponse> {
        self.check_access("op_env.commit", &req.common)?;

        self.next.commit(req).await
    }

    async fn abort(&self, req: OpEnvAbortInputRequest) -> BuckyResult<()> {
        self.check_access("op_env.abort", &req.common)?;

        self.next.abort(req).await
    }

    // map methods
    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyInputRequest,
    ) -> BuckyResult<OpEnvGetByKeyInputResponse> {
        self.check_access("op_env.get_by_key", &req.common)?;

        self.next.get_by_key(req).await
    }

    async fn insert_with_key(&self, req: OpEnvInsertWithKeyInputRequest) -> BuckyResult<()> {
        self.check_access("op_env.insert_with_key", &req.common)?;

        self.next.insert_with_key(req).await
    }

    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyInputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyInputResponse> {
        self.check_access("op_env.set_with_key", &req.common)?;

        self.next.set_with_key(req).await
    }

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyInputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyInputResponse> {
        self.check_access("op_env.remove_with_key", &req.common)?;

        self.next.remove_with_key(req).await
    }

    // set methods
    async fn contains(
        &self,
        req: OpEnvContainsInputRequest,
    ) -> BuckyResult<OpEnvContainsInputResponse> {
        self.check_access("op_env.contains", &req.common)?;

        self.next.contains(req).await
    }

    async fn insert(&self, req: OpEnvInsertInputRequest) -> BuckyResult<OpEnvInsertInputResponse> {
        self.check_access("op_env.insert", &req.common)?;

        self.next.insert(req).await
    }

    async fn remove(&self, req: OpEnvRemoveInputRequest) -> BuckyResult<OpEnvRemoveInputResponse> {
        self.check_access("op_env.remove", &req.common)?;

        self.next.remove(req).await
    }

    // iterator methods
    async fn next(&self, req: OpEnvNextInputRequest) -> BuckyResult<OpEnvNextInputResponse> {
        self.check_access("op_env.next", &req.common)?;

        self.next.next(req).await
    }

    async fn reset(&self, req: OpEnvResetInputRequest) -> BuckyResult<()> {
        self.check_access("op_env.reset", &req.common)?;

        self.next.reset(req).await
    }

    async fn list(&self, req: OpEnvListInputRequest) -> BuckyResult<OpEnvListInputResponse> {
        self.check_access("op_env.list", &req.common)?;

        self.next.list(req).await
    }

    // metadata
    async fn metadata(
        &self,
        req: OpEnvMetadataInputRequest,
    ) -> BuckyResult<OpEnvMetadataInputResponse> {
        self.check_access("op_env.metadata", &req.common)?;

        self.next.metadata(req).await
    }
}
