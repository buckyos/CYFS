use super::request::AclRequest;
use crate::router_handler::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::str::FromStr;

pub(crate) struct AclAccessHandler {
    acl_handlers: RouterHandlersContainerRef,
    id: String,
}

type RouterAclHandlerEmitter = RouterHandlerEmitter<AclHandlerRequest, AclHandlerResponse>;

impl AclAccessHandler {
    pub fn new(router_handlers: &RouterHandlersManager, id: &str) -> Self {
        Self {
            acl_handlers: router_handlers.handlers(&RouterHandlerChain::Acl).clone(),
            id: id.to_owned(),
        }
    }

    fn emitter(&self) -> Option<RouterAclHandlerEmitter> {
        let handlers = self.acl_handlers.try_acl();
        if let Some(handlers) = handlers {
            handlers.specified_emitter(&self.id)
        } else {
            None
        }
    }

    pub async fn emit(&self, req: &dyn AclRequest) -> BuckyResult<AclAccess> {
        match self.emitter() {
            Some(mut emitter) => {
                let param = req.handler_req().await;

                // next会根据handler filter再次做一次过滤，如果没有匹配上，那么该条acl被pass
                let resp = emitter.next(param, &RouterHandlerAction::Pass).await;
                match resp.action {
                    RouterHandlerAction::Response => match resp.response {
                        Some(ret) => match ret {
                            Ok(ret) => Ok(ret.access),
                            Err(e) => {
                                warn!("acl handler return error response! chain={}, category={}, req={}, err={}",
                                    emitter.chain(),
                                emitter.category(),
                                req.debug_info(), e);

                                Err(e)
                            }
                        },
                        None => {
                            let msg = format!(
                                "acl andler return resp action but empty response! chain={}, category={}, req={}",
                                emitter.chain(),
                                emitter.category(),
                                req.debug_info(),
                            );
                            warn!("{}", msg);
                            Err(BuckyError::new(BuckyErrorCode::InternalError, msg))
                        }
                    },
                    _ => Ok(Self::action_to_access(resp.action)),
                }
            }
            None => {
                let msg = format!("acl handler not found! id={}", self.id);
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }

    fn action_to_access(action: RouterHandlerAction) -> AclAccess {
        match action {
            RouterHandlerAction::Default => AclAccess::Reject,
            RouterHandlerAction::Drop => AclAccess::Drop,
            RouterHandlerAction::Reject => AclAccess::Reject,
            RouterHandlerAction::Pass => AclAccess::Pass,
            RouterHandlerAction::Response => unreachable!(),
        }
    }
}

pub(crate) enum AclAccessEx {
    Handler(AclAccessHandler),
    Access(AclAccess),
}

impl AclAccessEx {
    pub fn load(
        router_handlers: &RouterHandlersManager,
        id: &str,
        value: &str,
    ) -> BuckyResult<Self> {
        let ret = match value {
            "handler" => {
                let handler = AclAccessHandler::new(router_handlers, id);
                Self::Handler(handler)
            }
            _ => Self::Access(AclAccess::from_str(value)?),
        };

        Ok(ret)
    }

    pub async fn get_access(&self, req: &dyn AclRequest) -> BuckyResult<AclAccess> {
        match self {
            Self::Access(access) => Ok(access.clone()),
            Self::Handler(handler) => handler.emit(req).await,
        }
    }
}
