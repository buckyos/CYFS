// as miner in blockchain

use cyfs_base::{BuckyResult, ObjectId};
use cyfs_core::GroupProposal;

use crate::AsProposal;

#[async_trait::async_trait]
pub trait AsConsensus {
    async fn push_proposal(&self, proposal: GroupProposal) -> BuckyResult<()>;
    async fn get_nonce(&self, account: &ObjectId) -> BuckyResult<u64>;
}

pub(crate) trait AsConsensusInner {
    fn get_pending_proposal(&self) -> Vec<GroupProposal>;
    fn remove_proposal(&self, proposal_id: &ObjectId);
}

pub struct Consensus {}

impl Consensus {
    pub fn create() -> Self {
        Self {}
    }
}
