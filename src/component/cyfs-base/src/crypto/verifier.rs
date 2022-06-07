use crate::*;

use async_trait::async_trait;

#[async_trait]
pub trait Verifier: Send + Sync {
    fn public_key(&self) -> &PublicKey;
    async fn verify(&self, data: &[u8], sign: &Signature) -> bool;
}

#[async_trait]
pub trait PublicKeySearch: Send + Sync {
    async fn search_public_key(&self, sign: &Signature) -> BuckyResult<&PublicKey>;
}

#[async_trait]
impl Verifier for Box<dyn Verifier> {
    fn public_key(&self) -> &PublicKey {
        self.as_ref().public_key()
    }

    async fn verify(&self, data: &[u8], sign: &Signature) -> bool {
        self.as_ref().verify(data, sign).await
    }
}
