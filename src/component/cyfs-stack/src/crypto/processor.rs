use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait CryptoInputProcessor: Sync + Send + 'static {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse>;

    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse>;

    async fn encrypt_data(
        &self,
        req: CryptoEncryptDataInputRequest,
    ) -> BuckyResult<CryptoEncryptDataInputResponse>;

    async fn decrypt_data(
        &self,
        req: CryptoDecryptDataInputRequest,
    ) -> BuckyResult<CryptoDecryptDataInputResponse>;
}

pub type CryptoInputProcessorRef = Arc<Box<dyn CryptoInputProcessor>>;
