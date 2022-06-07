use super::handler::*;
use crate::non::*;
use crate::router_handler::{RouterHandlersContainer, RouterHandlersManager};
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct NONHandlerPreProcessor {
    next: NONInputProcessorRef,

    handlers: Arc<RouterHandlersContainer>,
}

impl NONHandlerPreProcessor {
    pub(crate) fn new(
        chain: RouterHandlerChain,
        next: NONInputProcessorRef,
        router_handlers: RouterHandlersManager,
    ) -> NONInputProcessorRef {
        let handlers = router_handlers
            .handlers(&chain)
            .clone();
        let ret = Self { next, handlers };

        Arc::new(Box::new(ret))
    }
                                
    pub async fn put_object(
        &self,
        request: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        let mut param = RouterHandlerPutObjectRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_put_object() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("put_object", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.put_object(param.request).await
    }

    pub async fn get_object(
        &self,
        request: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let mut param = RouterHandlerGetObjectRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_get_object() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("get_object", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.get_object(param.request).await
    }

    pub async fn post_object(
        &self,
        request: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        let mut param = RouterHandlerPostObjectRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_post_object() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("post_object", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.post_object(param.request).await
    }

    pub async fn select_object(
        &self,
        request: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        let mut param = RouterHandlerSelectObjectRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_select_object() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("select_object", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.select_object(param.request).await
    }

    pub async fn delete_object(
        &self,
        request: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        let mut param = RouterHandlerDeleteObjectRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_delete_object() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("delete_object", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.delete_object(param.request).await
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONHandlerPreProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        NONHandlerPreProcessor::put_object(&self, req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        NONHandlerPreProcessor::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        NONHandlerPreProcessor::post_object(&self, req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        NONHandlerPreProcessor::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        NONHandlerPreProcessor::delete_object(&self, req).await
    }
}
