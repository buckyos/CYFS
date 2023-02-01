use cyfs_base::{BuckyResult, Group, ObjectId};
use cyfs_core::{GroupConsensusBlock, GroupProposal};
use cyfs_lib::NONObjectInfo;

#[async_trait::async_trait]
pub trait DelegateFactory: Send + Sync {
    async fn create_rpath_delegate(
        &self,
        group: &Group,
        rpath: &str,
        with_block: Option<&GroupConsensusBlock>,
    ) -> BuckyResult<Box<dyn RPathDelegate>>;

    async fn on_state_changed(
        &self,
        group_id: &ObjectId,
        rpath: &str,
        state_id: Option<ObjectId>,
        pre_state_id: Option<ObjectId>,
    );
}

pub struct ExecuteResult {
    pub result_state_id: Option<ObjectId>, // pack block
    pub receipt: Option<NONObjectInfo>,    // to client
    pub context: Option<Vec<u8>>,          // timestamp etc.
}

#[async_trait::async_trait]
pub trait RPathDelegate: Sync + Send {
    async fn get_group(&self, group_chunk_id: Option<&ObjectId>) -> BuckyResult<Group>;

    async fn on_execute(
        &self,
        proposal: &GroupProposal,
        pre_state_id: Option<ObjectId>,
    ) -> BuckyResult<ExecuteResult>;

    async fn on_verify(
        &self,
        proposal: &GroupProposal,
        pre_state_id: Option<ObjectId>,
        execute_result: &ExecuteResult,
    ) -> BuckyResult<bool>;

    async fn on_commited(
        &self,
        proposal: &GroupProposal,
        pre_state_id: Option<ObjectId>,
        execute_result: &ExecuteResult,
        block: &GroupConsensusBlock,
    );
}
