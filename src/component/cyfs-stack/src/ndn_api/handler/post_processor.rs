use super::handler::*;
use crate::ndn::*;
use crate::router_handler::{RouterHandlersContainer, RouterHandlersManager};
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;


#[derive(Clone)]
pub(crate) struct NDNHandlerPostProcessor {
    next: NDNInputProcessorRef,

    handlers: Arc<RouterHandlersContainer>,
}

impl NDNHandlerPostProcessor {
    pub(crate) fn new(
        chain: RouterHandlerChain,
        next: NDNInputProcessorRef,
        router_handlers: RouterHandlersManager,
    ) -> NDNInputProcessorRef {
        let handlers = router_handlers
            .handlers(&chain)
            .clone();
        let ret = Self { next, handlers };

        Arc::new(Box::new(ret))
    }

    pub async fn put_data(
        &self,
        req: NDNPutDataInputRequest,
    ) -> BuckyResult<NDNPutDataInputResponse> {
        
        if let Some(handler) = self.handlers.try_put_data() {
            if !handler.is_empty() {

                let request = req.clone_without_data();
                let response = self.next.put_data(req).await;

                let mut param = RouterHandlerPutDataRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NDNHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("put_data", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.put_data(req).await
    }

    pub async fn get_data(
        &self,
        req: NDNGetDataInputRequest,
    ) -> BuckyResult<NDNGetDataInputResponse> {
        if let Some(handler) = self.handlers.try_get_data() {
            if !handler.is_empty() {

                let request = req.clone();
                let response = self.next.get_data(req).await;

                let mut param = RouterHandlerGetDataRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NDNHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("get_data", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.get_data(req).await
    }

    pub async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        if let Some(handler) = self.handlers.try_delete_data() {
            if !handler.is_empty() {

                let request = req.clone();
                let response = self.next.delete_data(req).await;

                let mut param = RouterHandlerDeleteDataRequest {
                    request,
                    response: Some(response),
                };

                let mut handler = NDNHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("delete_data", &mut param).await? {
                    return resp;
                }

                return param.response.unwrap();
            }
        }

        self.next.delete_data(req).await
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNHandlerPostProcessor {
    async fn put_data(
        &self,
        req: NDNPutDataInputRequest,
    ) -> BuckyResult<NDNPutDataInputResponse> {
        NDNHandlerPostProcessor::put_data(&self, req).await
    }

    async fn get_data(
        &self,
        req: NDNGetDataInputRequest,
    ) -> BuckyResult<NDNGetDataInputResponse> {
        NDNHandlerPostProcessor::get_data(&self, req).await
    }


    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        NDNHandlerPostProcessor::delete_data(&self, req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        // TODO query_file now not fire handler event
        self.next.query_file(req).await
    }
}
