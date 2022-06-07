use crate::crypto::*;
use crate::non_api::NONHandlerCaller;
use crate::router_handler::{RouterHandlersContainer, RouterHandlersManager};
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;


#[derive(Clone)]
pub(crate) struct CryptoHandlerPostProcessor {
    next: CryptoInputProcessorRef,

    handlers: Arc<RouterHandlersContainer>,
}

impl CryptoHandlerPostProcessor {
    pub(crate) fn new(
        chain: RouterHandlerChain,
        next: CryptoInputProcessorRef,
        router_handlers: RouterHandlersManager,
    ) -> CryptoInputProcessorRef {
        let handlers = router_handlers
            .handlers(&chain)
            .clone();
        let ret = Self { next, handlers };

        Arc::new(Box::new(ret))
    }

    pub async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        
        if let Some(handler) = self.handlers.try_sign_object() {
            if !handler.is_empty() {

                let request = req.clone();
                let response = self.next.sign_object(req).await;

                let mut param = RouterHandlerSignObjectRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("sign_object", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.sign_object(req).await
    }

    pub async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        if let Some(handler) = self.handlers.try_verify_object() {
            if !handler.is_empty() {

                let request = req.clone();
                let response = self.next.verify_object(req).await;

                let mut param = RouterHandlerVerifyObjectRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("verify_object", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.verify_object(req).await
    }
}

#[async_trait::async_trait]
impl CryptoInputProcessor for CryptoHandlerPostProcessor {
    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        CryptoHandlerPostProcessor::sign_object(&self, req).await
    }

    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        CryptoHandlerPostProcessor::verify_object(&self, req).await
    }
}
