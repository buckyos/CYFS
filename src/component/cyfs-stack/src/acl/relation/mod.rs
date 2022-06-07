mod creator;
mod desc;
mod device;
mod helper;
mod manager;
mod object;
mod cache;

pub(crate) use manager::*;

use super::request::AclRequest;
use cyfs_base::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub(crate) trait AclSpecifiedRelation: Sync + Send {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool>;
}


pub(crate) type AclSpecifiedRelationRef = Arc<Box<dyn AclSpecifiedRelation>>;