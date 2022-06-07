use super::output_request::*;
use cyfs_base::*;

use std::sync::Arc;


#[async_trait::async_trait]
pub trait NONOutputProcessor: Sync + Send + 'static {
    async fn put_object(
        &self,
        req: NONPutObjectOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse>;

    async fn get_object(&self, req: NONGetObjectOutputRequest)
        -> BuckyResult<NONGetObjectOutputResponse>;

    async fn post_object(&self, req: NONPostObjectOutputRequest)
        -> BuckyResult<NONPostObjectOutputResponse>;

    async fn select_object(&self, req: NONSelectObjectOutputRequest)
        -> BuckyResult<NONSelectObjectOutputResponse>;

    async fn delete_object(&self, req: NONDeleteObjectOutputRequest)
        -> BuckyResult<NONDeleteObjectOutputResponse>;
}

pub type NONOutputProcessorRef = Arc<Box<dyn NONOutputProcessor>>;

