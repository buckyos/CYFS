use crate::RequestSourceInfo;
use cyfs_base::*;

use std::sync::Arc;

pub struct GlobalStatePathHandlerRequest {
    // target_dec_id
    pub dec_id: ObjectId,

    // request source
    pub source: RequestSourceInfo,

    // full_req_path = {req_path}?{query_string}
    pub req_path: String,
    pub req_query_string: Option<String>,
    
    // The required permissions
    pub permissions: AccessPermissions,
}

#[async_trait::async_trait]
pub trait GlobalStatePathHandler: Sync + Send {
    async fn on_check(&self, req: GlobalStatePathHandlerRequest) -> BuckyResult<bool>;
}

pub type GlobalStatePathHandlerRef = Arc<Box<dyn GlobalStatePathHandler>>;
