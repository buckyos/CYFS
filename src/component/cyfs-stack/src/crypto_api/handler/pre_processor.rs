use crate::non_api::NONHandlerCaller;
use crate::crypto::*;
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
        let handlers = router_handlers
            .handlers(&chain)
            .clone();
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
}
