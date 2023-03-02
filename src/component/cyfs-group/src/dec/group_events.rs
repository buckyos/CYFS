use cyfs_base::{
    BuckyResult, Group, ObjectId, ObjectMapIsolatePathOpEnvRef, ObjectMapSingleOpEnvRef,
};
use cyfs_core::{GroupConsensusBlock, GroupProposal};
use cyfs_group_lib::ExecuteResult;
use cyfs_lib::NONObjectInfo;

#[derive(Clone)]
pub(crate) struct RPathEventNotifier {}

impl RPathEventNotifier {
    pub fn new() -> Self {
        unimplemented!()
    }

    pub async fn on_execute(
        &self,
        proposal: &GroupProposal,
        pre_state_id: Option<ObjectId>,
    ) -> BuckyResult<ExecuteResult> {
        unimplemented!()
    }

    pub async fn on_verify(
        &self,
        proposal: &GroupProposal,
        pre_state_id: Option<ObjectId>,
        execute_result: &ExecuteResult,
    ) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn on_commited(
        &self,
        proposal: &GroupProposal,
        pre_state_id: Option<ObjectId>,
        execute_result: &ExecuteResult,
        block: &GroupConsensusBlock,
    ) {
        unimplemented!()
    }
}
