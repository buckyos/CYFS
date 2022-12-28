use std::sync::Arc;

use cyfs_base::BuckyResult;
use cyfs_core::{GroupConsensusBlock, GroupProposal, GroupRPath, GroupRPathStatus};
use cyfs_lib::NONObjectInfo;

pub struct RPathClient {}

impl RPathClient {
    pub fn new() -> Self {
        Self {}
    }

    pub fn rpath(&self) -> &GroupRPath {
        unimplemented!()
    }

    pub async fn post_proposal(
        &self,
        proposal: GroupProposal,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        unimplemented!()
    }

    pub async fn get_field(&self, sub_path: &str) -> BuckyResult<GroupRPathStatus> {
        unimplemented!()
    }

    pub async fn get_block(&self, height: Option<u64>) -> BuckyResult<GroupConsensusBlock> {
        unimplemented!()
    }
}
