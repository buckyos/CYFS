use std::collections::HashMap;

use cyfs_base::{BuckyResult, GroupId, ObjectId};
use cyfs_core::DecAppId;

use crate::{DelegateFactory, IsCreateRPath, RPathClient, RPathControl};

type ByRPath = HashMap<String, RPathControl>;
type ByDec = HashMap<DecAppId, ByRPath>;
type ByGroup = HashMap<GroupId, ByDec>;

pub struct GroupRPathMgr {
    by_group: ByGroup,
}

impl GroupRPathMgr {
    pub fn new() -> Self {
        Self {
            by_group: ByGroup::default(),
        }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn close(&self) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn register(
        &self,
        dec_id: DecAppId,
        delegate_factory: Box<dyn DelegateFactory>,
    ) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn unregister(&self, dec_id: &DecAppId) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn find_rpath_control(
        &self,
        group_id: &GroupId,
        dec_id: &DecAppId,
        rpath: &str,
        is_auto_create: IsCreateRPath,
    ) -> BuckyResult<RPathControl> {
        unimplemented!()
    }

    pub async fn rpath_client(
        &self,
        group_id: &GroupId,
        dec_id: &DecAppId,
        rpath: &str,
    ) -> BuckyResult<RPathClient> {
        unimplemented!()
    }

    pub async fn rpath_control(
        &self,
        group_id: &GroupId,
        dec_id: &DecAppId,
        rpath: &str,
    ) -> BuckyResult<RPathControl> {
        unimplemented!()
    }

    pub async fn set_sync_path(&self, dec_id: &str, path: String) -> BuckyResult<()> {
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
