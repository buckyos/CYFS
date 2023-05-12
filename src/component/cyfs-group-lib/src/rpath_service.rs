use std::sync::Arc;

use cyfs_base::{BuckyResult, ObjectId};
use cyfs_core::{GroupConsensusBlock, GroupProposal, GroupProposalObject, GroupRPath};
use cyfs_lib::{
    HttpRequestorRef, IsolatePathOpEnvStub, NONObjectInfo, RootStateOpEnvAccess, SharedCyfsStack,
    SingleOpEnvStub,
};

use crate::{ExecuteResult, GroupObjectMapProcessor, GroupRequestor, RPathDelegate};

struct RPathServiceRaw {
    rpath: GroupRPath,
    requestor: GroupRequestor,
    delegate: Box<dyn RPathDelegate>,
    stack: SharedCyfsStack,
}

#[derive(Clone)]
pub struct RPathService(Arc<RPathServiceRaw>);

impl RPathService {
    pub fn rpath(&self) -> &GroupRPath {
        &self.0.rpath
    }

    pub async fn push_proposal(
        &self,
        proposal: &GroupProposal,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        // post http
        self.0
            .requestor
            .push_proposal(
                GroupRequestor::make_default_common(proposal.rpath().dec_id().clone()),
                proposal,
            )
            .await
            .map(|resp| resp.object)
    }

    pub(crate) fn new(
        rpath: GroupRPath,
        requestor: HttpRequestorRef,
        delegate: Box<dyn RPathDelegate>,
        stack: SharedCyfsStack,
    ) -> Self {
        Self(Arc::new(RPathServiceRaw {
            requestor: GroupRequestor::new(rpath.dec_id().clone(), requestor),
            rpath,
            delegate,
            stack,
        }))
    }

    pub(crate) async fn start(&self) -> BuckyResult<()> {
        // post create command
        self.0
            .requestor
            .start_service(
                GroupRequestor::make_default_common(self.0.rpath.dec_id().clone()),
                self.rpath().group_id(),
                self.rpath().rpath(),
            )
            .await
            .map(|_| {})
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
        prev_state_id: &Option<ObjectId>,
        block: &GroupConsensusBlock,
    ) {
        self.0
            .delegate
            .on_commited(
                prev_state_id,
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
