use crate::non_api::{NONHandlerCaller, RequestHandlerHelper};
use crate::router_handler::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// acl
impl RequestHandlerHelper<AclHandlerRequest> for AclHandlerRequest {
    fn update(&mut self, handler: Self) {
        self.req_path = handler.req_path;
        self.dec_id = handler.dec_id;
        self.permissions = handler.permissions;
    }

    fn debug_info(&self) -> String {
        self.req_path.clone()
    }

    fn req_path(&self) -> Option<&String> {
        Some(&self.req_path)
    }

    fn source(&self) -> &RequestSourceInfo {
        &self.source
    }
}
impl RequestHandlerHelper<Self> for AclHandlerResponse {
    fn update(&mut self, handler: Self) {
        self.action = handler.action;
    }
}

pub struct AclHandlerWrapper {
    handlers: Arc<RouterHandlersContainer>,
}

impl AclHandlerWrapper {
    pub fn new(router_handlers: &RouterHandlersManager) -> Self {
        Self {
            handlers: router_handlers.handlers(&RouterHandlerChain::Acl).clone(),
        }
    }
}

#[async_trait::async_trait]
impl GlobalStatePathHandler for AclHandlerWrapper {
    async fn on_check(&self, req: GlobalStatePathHandlerRequest) -> BuckyResult<bool> {
        let request = AclHandlerRequest {
            dec_id: req.dec_id,
            source: req.source,
            req_path: req.req_path,
            req_query_string: req.req_query_string,
            permissions: req.permissions,
        };

        let mut param = RouterHandlerAclRequest {
            request,
            response: None,
        };

        if let Some(handler) = self.handlers.try_acl() {
            if !handler.is_empty() {
                let mut handler = NONHandlerCaller::new(handler.emitter());
                if let Some(ret) = handler.call("acl", &mut param).await? {
                    let ret = match ret {
                        Ok(resp) => match resp.action {
                            AclAction::Accept => Ok(true),
                            AclAction::Reject => Ok(false),
                        },
                        Err(e) => Err(e),
                    };

                    return ret;
                }
            }
        }

        Ok(false)
    }
}
