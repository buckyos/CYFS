use std::collections::HashMap;

use cyfs_base::BuckyResult;
use cyfs_core::DecAppId;

use crate::GroupRPathMgr;

type ByDec = HashMap<DecAppId, Vec<GroupRPathMgr>>;

pub struct RPathControlMgr {
    by_dec: ByDec,
}

impl RPathControlMgr {
    pub async fn start(&self) -> BuckyResult<Self> {
        unimplemented!()
    }

    pub async fn close(&self) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn create_rpath_mgr_with_dec_id(
        &self,
        dec_id: &DecAppId,
    ) -> BuckyResult<GroupRPathMgr> {
        unimplemented!()
    }
}
