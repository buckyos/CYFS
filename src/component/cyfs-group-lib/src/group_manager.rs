use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use cyfs_core::DecAppId;
use cyfs_lib::SharedCyfsStack;

use crate::{DelegateFactory, RPathClient, RPathDelegate, RPathService};

#[derive(Clone)]
pub struct GroupManager;

impl GroupManager {
    pub async fn open(
        stack: SharedCyfsStack,
        delegate_factory: Box<dyn DelegateFactory>,
    ) -> BuckyResult<Self> {
        unimplemented!()
    }

    pub async fn open_as_client(stack: SharedCyfsStack) -> BuckyResult<Self> {
        unimplemented!()
    }

    pub async fn start_rpath_service(
        &self,
        group_id: ObjectId,
        rpath: String,
        delegate: Box<dyn RPathDelegate>,
    ) -> BuckyResult<RPathService> {
        Err(BuckyError::new(BuckyErrorCode::NotImplement, ""))
    }

    pub async fn find_rpath_service(
        &self,
        group_id: ObjectId,
        rpath: String,
    ) -> BuckyResult<RPathService> {
        unimplemented!()
    }

    pub async fn rpath_client(
        &self,
        group_id: ObjectId,
        dec_id: DecAppId,
        rpath: String,
    ) -> BuckyResult<RPathClient> {
        unimplemented!()
    }
}
