use super::obj_signer::*;
use super::obj_verifier::*;
use crate::crypto::*;
use crate::resolver::*;
use crate::zone::*;
use cyfs_lib::*;
use cyfs_bdt::StackGuard;
use cyfs_base::*;


use std::sync::Arc;

#[derive(Clone)]
pub struct ObjectCrypto {
    signer: Arc<ObjectSigner>,
    verifier: Arc<ObjectVerifier>,
}

impl ObjectCrypto {
    pub(crate) fn new(
        verifier: Arc<ObjectVerifier>,
        zone_manager: ZoneManager,
        device_manager: Box<dyn DeviceCache>,
        bdt_stack: StackGuard,
    ) -> Self {
        let signer = ObjectSigner::new(
            zone_manager.clone(),
            device_manager.clone_cache(),
            &bdt_stack,
        );

        let signer = Arc::new(signer);

        Self { signer, verifier }
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
}
