use cyfs_base::BuckyResult;
use cyfs_core::GroupConsensusBlock;

pub struct OrderBlockMgr {}

impl OrderBlockMgr {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn push_block(&self, block: &GroupConsensusBlock) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn pop_link(&self, block: &GroupConsensusBlock) -> BuckyResult<()> {
        unimplemented!()
    }
}
