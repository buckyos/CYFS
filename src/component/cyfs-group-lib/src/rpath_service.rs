use std::sync::Arc;

use cyfs_base::{BuckyResult, ObjectId};
use cyfs_core::{GroupConsensusBlock, GroupProposal, GroupRPath};
use cyfs_lib::{
    HttpRequestorRef, IsolatePathOpEnvStub, RootStateOpEnvAccess, SharedCyfsStack, SingleOpEnvStub,
};

use crate::{ExecuteResult, GroupObjectMapProcessor, RPathDelegate};

struct RPathServiceRaw {
    rpath: GroupRPath,
    requestor: HttpRequestorRef,
    delegate: Box<dyn RPathDelegate>,
    stack: SharedCyfsStack,
}

#[derive(Clone)]
pub struct RPathService(Arc<RPathServiceRaw>);

impl RPathService {
    pub fn rpath(&self) -> &GroupRPath {
        unimplemented!()
    }

    pub async fn push_proposal(&self, proposal: &GroupProposal) -> BuckyResult<()> {
        // post http
        unimplemented!()
    }

    pub(crate) fn new(
        rpath: GroupRPath,
        requestor: HttpRequestorRef,
        delegate: Box<dyn RPathDelegate>,
        stack: SharedCyfsStack,
    ) -> Self {
        Self(Arc::new(RPathServiceRaw {
            rpath,
            requestor,
            delegate,
            stack,
        }))
    }

    pub(crate) async fn start(&self) -> BuckyResult<Self> {
        // post create command
        unimplemented!()
    }

    pub(crate) async fn on_execute(
        &self,
        proposal: &GroupProposal,
        prev_state_id: &Option<ObjectId>,
    ) -> BuckyResult<ExecuteResult> {
        self.0
            .delegate
            .on_execute(
                proposal,
                prev_state_id,
                &GroupObjectMapProcessorImpl {
                    stack: self.0.stack.clone(),
                },
            )
            .await
    }

    pub(crate) async fn on_verify(
        &self,
        proposal: &GroupProposal,
        prev_state_id: &Option<ObjectId>,
        execute_result: &ExecuteResult,
    ) -> BuckyResult<()> {
        self.0
            .delegate
            .on_verify(
                proposal,
                prev_state_id,
                execute_result,
                &GroupObjectMapProcessorImpl {
                    stack: self.0.stack.clone(),
                },
            )
            .await
    }

    pub(crate) async fn on_commited(
        &self,
        proposal: &GroupProposal,
        prev_state_id: &Option<ObjectId>,
        execute_result: &ExecuteResult,
        block: &GroupConsensusBlock,
    ) {
        self.0
            .delegate
            .on_commited(
                proposal,
                prev_state_id,
                execute_result,
                block,
                &GroupObjectMapProcessorImpl {
                    stack: self.0.stack.clone(),
                },
            )
            .await
    }
}

struct GroupObjectMapProcessorImpl {
    stack: SharedCyfsStack,
}

#[async_trait::async_trait]
impl GroupObjectMapProcessor for GroupObjectMapProcessorImpl {
    async fn create_single_op_env(
        &self,
        access: Option<RootStateOpEnvAccess>,
    ) -> BuckyResult<SingleOpEnvStub> {
        self.stack
            .root_state_stub(None, None)
            .create_single_op_env_with_access(access)
            .await
    }

    async fn create_sub_tree_op_env(
        &self,
        access: Option<RootStateOpEnvAccess>,
    ) -> BuckyResult<IsolatePathOpEnvStub> {
        self.stack
            .root_state_stub(None, None)
            .create_isolate_path_op_env_with_access(access)
            .await
    }
}
