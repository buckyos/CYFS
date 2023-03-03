use std::sync::Arc;

use cyfs_base::BuckyResult;
use cyfs_core::{GroupProposal, GroupRPath};

struct RPathServiceRaw {}

#[derive(Clone)]
pub struct RPathService(Arc<RPathServiceRaw>);

impl RPathService {
    pub(crate) async fn load() -> BuckyResult<Self> {
        unimplemented!()
    }

    pub fn rpath(&self) -> &GroupRPath {
        unimplemented!()
    }

    pub async fn push_proposal(&self, proposal: &GroupProposal) -> BuckyResult<()> {
        unimplemented!()
    }
}
