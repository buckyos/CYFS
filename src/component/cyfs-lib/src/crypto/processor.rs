use super::output_request::*;
use cyfs_base::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait CryptoOutputProcessor: Sync + Send + 'static {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectOutputRequest,
    ) -> BuckyResult<CryptoVerifyObjectOutputResponse>;

    async fn sign_object(
        &self,
        req: CryptoSignObjectOutputRequest,
    ) -> BuckyResult<CryptoSignObjectOutputResponse>;

    async fn encrypt_data(
        &self,
        req: CryptoEncryptDataOutputRequest,
    ) -> BuckyResult<CryptoEncryptDataOutputResponse>;

    async fn decrypt_data(
        &self,
        req: CryptoDecryptDataOutputRequest,
    ) -> BuckyResult<CryptoDecryptDataOutputResponse>;
}

pub type CryptoOutputProcessorRef = Arc<Box<dyn CryptoOutputProcessor>>;
