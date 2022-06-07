use crate::*;

use async_trait::async_trait;

#[async_trait]
pub trait Signer: Sync + Send {
    fn public_key(&self) -> &PublicKey;
    async fn sign(&self, data: &[u8], sign_source: &SignatureSource) -> BuckyResult<Signature>;
}

#[async_trait]
impl Signer for Box<dyn Signer> {
    fn public_key(&self) -> &PublicKey {
        self.as_ref().public_key()
    }

    async fn sign(&self, data: &[u8], sign_source: &SignatureSource) -> BuckyResult<Signature> {
        self.as_ref().sign(data, sign_source).await
    }
}
