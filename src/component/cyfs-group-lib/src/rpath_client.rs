use std::sync::Arc;

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, GroupMemberScope, NamedObject, ObjectDesc, ObjectId,
    RawConvertTo,
};
use cyfs_core::{GroupProposal, GroupRPath};
use cyfs_lib::NONObjectInfo;

struct RPathClientRaw {}

#[derive(Clone)]
pub struct RPathClient(Arc<RPathClientRaw>);

impl RPathClient {
    pub fn rpath(&self) -> &GroupRPath {
        unimplemented!()
    }

    pub async fn post_proposal(
        &self,
        proposal: &GroupProposal,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        unimplemented!()
    }
}
