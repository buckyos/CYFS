use cyfs_base::{BuckyResult, Group, ObjectId};
use cyfs_core::GroupProposal;
use cyfs_lib::NONObjectInfo;

#[async_trait::async_trait]
pub trait DelegateFactory {
    async fn create_rpath_delegate(
        &self,
        group: &Group,
        dec_id: &ObjectId,
        rpath: &str,
        with_proposal: Option<&GroupProposal>,
    ) -> BuckyResult<Box<dyn RPathDelegate>>;
}

pub struct ExecuteResult {
    pub result_state_id: ObjectId,            // pack block
    pub receipt: Option<NONObjectInfo>, // to client
    pub context: Vec<u8>,                     // timestamp etc.
}

#[async_trait::async_trait]
pub trait RPathDelegate: Sync + Send {
    async fn get_group(&self, group_chunk_id: Option<&ObjectId>) -> BuckyResult<Group>;

    async fn on_execute(
        &self,
        proposal: &GroupProposal,
        pre_state_id: ObjectId,
    ) -> BuckyResult<ExecuteResult>;

    async fn on_verify(
        &self,
        proposal: &GroupProposal,
        pre_state_id: ObjectId,
        execute_result: &ExecuteResult,
    ) -> BuckyResult<bool>;

    async fn on_commited(
        &self,
        proposal: &GroupProposal,
        pre_state_id: ObjectId,
        execute_result: &ExecuteResult,
    );
}
