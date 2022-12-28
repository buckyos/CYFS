use cyfs_base::BuckyResult;
use cyfs_core::{GroupConsensusBlock, GroupProposal};

pub trait AsBlock {}

#[async_trait::async_trait]
pub trait BlockBuilder {
    async fn build(
        &self,
        proposals: Vec<GroupProposal>,
    ) -> BuckyResult<Option<GroupConsensusBlock>>;
}

impl AsBlock for GroupConsensusBlock {}

pub struct GroupBlockBuilder {}

#[async_trait::async_trait]
impl BlockBuilder for GroupBlockBuilder {}
