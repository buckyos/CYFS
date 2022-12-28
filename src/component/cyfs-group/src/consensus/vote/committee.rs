use cyfs_base::{BuckyResult, Group, ObjectId};
use cyfs_core::GroupConsensusBlock;

pub struct Committee {}

impl Committee {
    pub fn spawn() {}

    pub async fn get_group(&self, group_chunk_id: &ObjectId) -> BuckyResult<Group> {
        unimplemented!()
    }

    pub async fn quorum_threshold(
        &self,
        voters: &[ObjectId],
        block_id: &ObjectId,
    ) -> BuckyResult<bool> {
        unimplemented!()
    }

    pub async fn timeout_threshold(
        &self,
        voters: &[ObjectId],
        high_qc_block_id: &ObjectId,
    ) -> BuckyResult<u32> {
        unimplemented!()
    }

    pub async fn get_leader(&self, group_chunk_id: &ObjectId) -> BuckyResult<ObjectId> {
        unimplemented!()
    }

    pub async fn verify_block(&self, block: &GroupConsensusBlock) -> BuckyResult<()> {
        unimplemented!()
        /* *
         * 验证block下的签名是否符合对上一个block归属group的确认
         */
    }
}
