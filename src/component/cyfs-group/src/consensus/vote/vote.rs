use cyfs_base::{BuckyResult, ObjectId, Signature};
use cyfs_core::GroupConsensusBlock;

use super::committee::Committee;

impl HotstuffBlockQCVote {
    pub async fn new(
        block: &GroupConsensusBlock,
        voter: ObjectId, /*stack*/
    ) -> BuckyResult<Vote> {
        unimplemented!()
    }

    pub async fn verify(&self, committee: &Committee /*stack*/) -> BuckyResult<bool> {
        unimplemented!()
    }
}
