use super::output_request::*;
use cyfs_base::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait NDNOutputProcessor: Sync + Send + 'static {
    async fn put_data(&self, req: NDNPutDataOutputRequest)
        -> BuckyResult<NDNPutDataOutputResponse>;

    async fn get_data(&self, req: NDNGetDataOutputRequest)
        -> BuckyResult<NDNGetDataOutputResponse>;

    async fn put_shared_data(&self, req: NDNPutDataOutputRequest)
                      -> BuckyResult<NDNPutDataOutputResponse>;

    async fn get_shared_data(&self, req: NDNGetDataOutputRequest)
                      -> BuckyResult<NDNGetDataOutputResponse>;

    async fn delete_data(
        &self,
        req: NDNDeleteDataOutputRequest,
    ) -> BuckyResult<NDNDeleteDataOutputResponse>;

    async fn query_file(
        &self,
        req: NDNQueryFileOutputRequest,
    ) -> BuckyResult<NDNQueryFileOutputResponse>;
}

pub type NDNOutputProcessorRef = Arc<Box<dyn NDNOutputProcessor>>;
