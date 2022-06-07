use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub(crate) trait NDNInputProcessor: Sync + Send {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse>;

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse>;

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse>;

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse>;
}

pub(crate) type NDNInputProcessorRef = Arc<Box<dyn NDNInputProcessor>>;
