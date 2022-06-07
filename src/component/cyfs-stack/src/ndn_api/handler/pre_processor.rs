use super::handler::*;
use crate::ndn::*;
use crate::router_handler::{RouterHandlersContainer, RouterHandlersManager};
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct NDNHandlerPreProcessor {
    next: NDNInputProcessorRef,

    handlers: Arc<RouterHandlersContainer>,
}

impl NDNHandlerPreProcessor {
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
        request: NDNPutDataInputRequest,
    ) -> BuckyResult<NDNPutDataInputResponse> {
        let mut param = RouterHandlerPutDataRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_put_data() {
            if !handler.is_empty() {
                let mut handler = NDNHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("put_data", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.put_data(param.request).await
    }

    pub async fn get_data(
        &self,
        request: NDNGetDataInputRequest,
    ) -> BuckyResult<NDNGetDataInputResponse> {
        let mut param = RouterHandlerGetDataRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_get_data() {
            if !handler.is_empty() {
                let mut handler = NDNHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("get_data", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.get_data(param.request).await
    }


    pub async fn delete_data(
        &self,
        request: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        let mut param = RouterHandlerDeleteDataRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_delete_data() {
            if !handler.is_empty() {
                let mut handler = NDNHandlerCaller::new(handler.emitter());
                if let Some(resp) = handler.call("delete_data", &mut param).await? {
                    return resp;
                }
            }
        }

        self.next.delete_data(param.request).await
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNHandlerPreProcessor {
    async fn put_data(
        &self,
        req: NDNPutDataInputRequest,
    ) -> BuckyResult<NDNPutDataInputResponse> {
        NDNHandlerPreProcessor::put_data(&self, req).await
    }

    async fn get_data(
        &self,
        req: NDNGetDataInputRequest,
    ) -> BuckyResult<NDNGetDataInputResponse> {
        NDNHandlerPreProcessor::get_data(&self, req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        NDNHandlerPreProcessor::delete_data(&self, req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        // TODO query_file now not fire handler event
        self.next.query_file(req).await
    }
}
