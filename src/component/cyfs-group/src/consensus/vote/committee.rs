use std::collections::HashSet;

use cyfs_base::{BuckyResult, Group, ObjectId};
use cyfs_core::{GroupConsensusBlock, HotstuffTimeout};

use crate::{HotstuffBlockQCVote, HotstuffTimeoutVote};

#[derive(Clone)]
pub struct Committee {}

impl Committee {
    pub fn spawn() {}

    pub async fn get_group(&self, group_chunk_id: Option<&ObjectId>) -> BuckyResult<Group> {
        unimplemented!()
    }

    pub async fn quorum_threshold(
        &self,
        voters: &HashSet<ObjectId>,
        group_chunk_id: Option<&ObjectId>,
    ) -> BuckyResult<bool> {
        unimplemented!()
    }

    pub async fn get_leader(
        &self,
        group_chunk_id: Option<&ObjectId>,
        round: u64,
    ) -> BuckyResult<ObjectId> {
        unimplemented!()
    }

    pub async fn verify_block(&self, block: &GroupConsensusBlock) -> BuckyResult<()> {
        unimplemented!()
        /* *
         * 验证block下的签名是否符合对上一个block归属group的确认
         */
    }

    pub async fn verify_vote(&self, vote: &HotstuffBlockQCVote) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn verify_timeout(
        &self,
        vote: &HotstuffTimeoutVote,
        pre_block: Option<&GroupConsensusBlock>,
    ) -> BuckyResult<()> {
        // 用block验vote.high_qc
        unimplemented!()
    }

    pub async fn verify_tc(
        &self,
        tc: &HotstuffTimeout,
        pre_block: Option<&GroupConsensusBlock>,
    ) -> BuckyResult<()> {
        // 用block验vote.high_qc
        unimplemented!()
    }
}
