use cyfs_base::{BuckyResult, ObjectId};

use crate::{DelegateFactory, RPathClient};

#[derive(Clone)]
pub struct GroupManager;

impl GroupManager {
    pub async fn rpath_client(&self, group_id: &ObjectId, rpath: &str) -> BuckyResult<RPathClient> {
        unimplemented!()
    }

    pub async fn register(&self, delegate_factory: Box<dyn DelegateFactory>) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn unregister(&self) {
        unimplemented!()
    }
}
