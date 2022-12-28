use std::collections::HashMap;

use cyfs_base::{BuckyResult, GroupId, ObjectId};
use cyfs_core::{DecAppId, GroupRPath};

use crate::{DelegateFactory, IsCreateRPath, RPathClient, RPathControl};

type ByRPath = HashMap<String, BftConsensus>;
type ByDec = HashMap<DecAppId, ByRPath>;
type ByGroup = HashMap<GroupId, ByDec>;

pub struct RPathControlMgr {
    dec_id: DecAppId,
    by_group: ByGroup,
}

impl RPathControlMgr {
    pub fn new(dec_id: DecAppId) -> Self {
        Self {
            by_group: ByGroup::default(),
            dec_id,
        }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn close(&self) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn register(&self, delegate_factory: Box<dyn DelegateFactory>) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn find_rpath_control(
        &self,
        group_id: &GroupId,
        rpath: &str,
        is_auto_create: IsCreateRPath,
    ) -> BuckyResult<RPathControl> {
        unimplemented!()
    }

    pub async fn rpath_client(&self, group_id: &GroupId, rpath: &str) -> BuckyResult<RPathClient> {
        unimplemented!()
    }

    // return Vec<GroupId>
    pub async fn enum_group(&self) -> BuckyResult<Vec<GroupId>> {
        unimplemented!()
    }

    // return <DecId, RPath>
    pub async fn enum_rpath_control(
        &self,
        group_id: &ObjectId,
    ) -> BuckyResult<Vec<(DecAppId, String)>> {
        unimplemented!()
    }
}
