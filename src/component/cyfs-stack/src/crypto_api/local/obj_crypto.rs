use super::obj_signer::*;
use super::obj_verifier::*;
use crate::crypto::*;
use crate::resolver::*;
use crate::zone::*;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;
use super::codec::CryptoCodec;

use std::sync::Arc;

#[derive(Clone)]
pub struct ObjectCrypto {
    signer: Arc<ObjectSigner>,
    verifier: Arc<ObjectVerifier>,
    codec: Arc<CryptoCodec>,
}

impl ObjectCrypto {
    pub(crate) fn new(
        verifier: Arc<ObjectVerifier>,
        zone_manager: ZoneManagerRef,
        device_manager: Box<dyn DeviceCache>,
        bdt_stack: StackGuard,
    ) -> Self {
        let signer = ObjectSigner::new(
            zone_manager.clone(),
            device_manager.clone_cache(),
            &bdt_stack,
        );

        let signer = Arc::new(signer);

        let codec = CryptoCodec::new(zone_manager, bdt_stack);
        let codec = Arc::new(codec);

        Self { signer, verifier, codec }
    }

    pub fn clone_processor(&self) -> CryptoInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    pub fn signer(&self) -> &Arc<ObjectSigner> {
        return &self.signer;
    }

    pub fn verifier(&self) -> &Arc<ObjectVerifier> {
        return &self.verifier;
    }
}

#[async_trait::async_trait]
impl CryptoInputProcessor for ObjectCrypto {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        self.verifier.verify_object(req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        self.signer.sign_object(req).await
    }

    async fn encrypt_data(
        &self,
        req: CryptoEncryptDataInputRequest,
    ) -> BuckyResult<CryptoEncryptDataInputResponse> {
        self.codec.encrypt_data(req).await
    }

    async fn decrypt_data(
        &self,
        req: CryptoDecryptDataInputRequest,
    ) -> BuckyResult<CryptoDecryptDataInputResponse> {
        self.codec.decrypt_data(req).await
    }
}