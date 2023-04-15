use crate::RequestSourceInfo;
use cyfs_base::*;

use std::sync::Arc;

pub struct GlobalStatePathHandlerRequest {
    pub dec_id: ObjectId,
    pub req_path: String,
    pub source: RequestSourceInfo,

    // The required permissions
    pub permissions: AccessPermissions,
}

#[async_trait::async_trait]
pub trait GlobalStatePathHandler: Sync + Send {
    async fn on_check(&self, req: GlobalStatePathHandlerRequest) -> BuckyResult<bool>;
}

pub type GlobalStatePathHandlerRef = Arc<Box<dyn GlobalStatePathHandler>>;
