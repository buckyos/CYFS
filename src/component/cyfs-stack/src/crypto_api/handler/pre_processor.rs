use crate::crypto::*;
use crate::non_api::NONHandlerCaller;
use crate::router_handler::{RouterHandlersContainer, RouterHandlersManager};
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct CryptoHandlerPreProcessor {
    next: CryptoInputProcessorRef,

    handlers: Arc<RouterHandlersContainer>,
}

impl CryptoHandlerPreProcessor {
    pub(crate) fn new(
        chain: RouterHandlerChain,
        next: CryptoInputProcessorRef,
        router_handlers: RouterHandlersManager,
    ) -> CryptoInputProcessorRef {
        let handlers = router_handlers.handlers(&chain).clone();
        let ret = Self { next, handlers };

        Arc::new(Box::new(ret))
    }

    pub async fn sign_object(
        &self,
        request: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        let mut param = RouterHandlerSignObjectRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_sign_object() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("sign_object", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.sign_object(param.request).await
    }

    pub async fn verify_object(
        &self,
        request: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        let mut param = RouterHandlerVerifyObjectRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_verify_object() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("verify_object", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.verify_object(param.request).await
    }

    pub async fn encrypt_data(
        &self,
        request: CryptoEncryptDataInputRequest,
    ) -> BuckyResult<CryptoEncryptDataInputResponse> {
        let mut param = RouterHandlerEncryptDataRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_encrypt_data() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("encrypt_data", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.encrypt_data(param.request).await
    }

    pub async fn decrypt_data(
        &self,
        request: CryptoDecryptDataInputRequest,
    ) -> BuckyResult<CryptoDecryptDataInputResponse> {
        let mut param = RouterHandlerDecryptDataRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_decrypt_data() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("decrypt_data", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.decrypt_data(param.request).await
    }
}

#[async_trait::async_trait]
impl CryptoInputProcessor for CryptoHandlerPreProcessor {
    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        CryptoHandlerPreProcessor::sign_object(&self, req).await
    }

    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        CryptoHandlerPreProcessor::verify_object(&self, req).await
    }

    async fn encrypt_data(
        &self,
        req: CryptoEncryptDataInputRequest,
    ) -> BuckyResult<CryptoEncryptDataInputResponse> {
        Self::encrypt_data(&self, req).await
    }

    async fn decrypt_data(
        &self,
        req: CryptoDecryptDataInputRequest,
    ) -> BuckyResult<CryptoDecryptDataInputResponse> {
        Self::decrypt_data(&self, req).await
    }
}
