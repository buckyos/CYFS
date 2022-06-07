use cyfs_lib::*;
use cyfs_base::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait CryptoInputProcessor: Sync + Send + 'static {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse>;

    async fn sign_object(&self, req: CryptoSignObjectInputRequest)
        -> BuckyResult<CryptoSignObjectInputResponse>;
}

pub type CryptoInputProcessorRef = Arc<Box<dyn CryptoInputProcessor>>;