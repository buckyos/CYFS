use super::handler::*;
use crate::non::*;
use crate::router_handler::{RouterHandlersContainer, RouterHandlersManager};
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;


#[derive(Clone)]
pub(crate) struct NONHandlerPostProcessor {
    next: NONInputProcessorRef,

    handlers: Arc<RouterHandlersContainer>,
}

impl NONHandlerPostProcessor {
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
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        
        if let Some(handler) = self.handlers.try_put_object() {
            if !handler.is_empty() {

                let request = req.clone();
                let response = self.next.put_object(req).await;

                let mut param = RouterHandlerPutObjectRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("put_object", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.put_object(req).await
    }

    pub async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        if let Some(handler) = self.handlers.try_get_object() {
            if !handler.is_empty() {

                let request = req.clone();
                let response = self.next.get_object(req).await;

                let mut param = RouterHandlerGetObjectRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("get_object", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.get_object(req).await
    }

    pub async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        if let Some(handler) = self.handlers.try_post_object() {
            if !handler.is_empty() {

                let request = req.clone();
                let response = self.next.post_object(req).await;

                let mut param = RouterHandlerPostObjectRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("post_object", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.post_object(req).await
    }

    pub async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        if let Some(handler) = self.handlers.try_select_object() {
            if !handler.is_empty() {

                let request = req.clone();
                let response = self.next.select_object(req).await;

                let mut param = RouterHandlerSelectObjectRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("select_object", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.select_object(req).await
    }

    pub async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        if let Some(handler) = self.handlers.try_delete_object() {
            if !handler.is_empty() {

                let request = req.clone();
                let response = self.next.delete_object(req).await;

                let mut param = RouterHandlerDeleteObjectRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("delete_object", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.delete_object(req).await
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONHandlerPostProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        NONHandlerPostProcessor::put_object(&self, req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        NONHandlerPostProcessor::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        NONHandlerPostProcessor::post_object(&self, req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        NONHandlerPostProcessor::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        NONHandlerPostProcessor::delete_object(&self, req).await
    }
}
