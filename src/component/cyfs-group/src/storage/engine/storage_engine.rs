use cyfs_base::{BuckyResult, ObjectId};

#[async_trait::async_trait]
pub trait StorageWriter: Send + Sync {
    async fn insert_prepares(&mut self, block_id: &ObjectId) -> BuckyResult<()>;
    async fn insert_pre_commit(&mut self, block_id: &ObjectId, is_instead: bool)
        -> BuckyResult<()>;
    async fn push_commit(
        &mut self,
        height: u64,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
        prev_result_state_id: &Option<ObjectId>,
        min_height: u64,
    ) -> BuckyResult<()>;
    async fn remove_prepares(&mut self, block_ids: &[ObjectId]) -> BuckyResult<()>;
    async fn push_proposals(
        &mut self,
        proposal_ids: &[ObjectId],
        timestamp: Option<(u64, u64)>, // (timestamp, prev_timestamp), 0 if the first
    ) -> BuckyResult<()>;

    async fn set_last_vote_round(&mut self, round: u64, prev_value: u64) -> BuckyResult<()>;

    async fn commit(mut self) -> BuckyResult<()>;
}

#[async_trait::async_trait]
pub trait StorageEngine {
    async fn find_block_by_height(&self, height: u64) -> BuckyResult<ObjectId>;
    // async fn is_proposal_finished(&self, proposal_id: &ObjectId) -> BuckyResult<bool>;
}
