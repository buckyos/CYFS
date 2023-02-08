use super::super::handler::*;
use crate::router_handler::{RouterHandlersContainer, RouterHandlersManager};
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct NONRouterHandler {
    handlers: Arc<RouterHandlersContainer>,
    zone_manager: ZoneManagerRef,
}

impl NONRouterHandler {
    pub(crate) fn new(
        router_handlers: &RouterHandlersManager,
        zone_manager: ZoneManagerRef,
    ) -> Self {
        let handlers = router_handlers
            .handlers(&RouterHandlerChain::Handler)
            .clone();

        Self {
            handlers,
            zone_manager,
        }
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

        let msg = format!(
            "post object must be handled by handler! req={}, source: {}, device={}",
            param.request.object.object_id,
            param.request.common.source,
            self.zone_manager.get_current_device_id(),
        );
        warn!("{}", msg);

        Err(BuckyError::new(BuckyErrorCode::NotHandled, msg))
    }
}
