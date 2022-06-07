use cyfs_lib::*;
use cyfs_base::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub(crate) trait NONInputProcessor: Sync + Send + 'static {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse>;

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse>;

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse>;

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse>;

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse>;
}

pub(crate) type NONInputProcessorRef = Arc<Box<dyn NONInputProcessor>>;