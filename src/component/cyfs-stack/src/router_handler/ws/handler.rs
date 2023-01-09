use super::super::RouterHandlersManager;
use super::processor::*;
use crate::interface::{HttpRequestSource, InterfaceAuth};
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct RouterHandlerWebSocketHandler {
    protocol: RequestProtocol,

    processor: RouterHandlerWSProcessor,

    zone_manager: ZoneManagerRef,
}

impl RouterHandlerWebSocketHandler {
    pub fn new(protocol: RequestProtocol, manager: RouterHandlersManager) -> Self {
        let zone_manager = manager.acl_manager().zone_manager().clone();
        let processor = RouterHandlerWSProcessor::new(manager);
        Self {
            protocol,
            processor,
            zone_manager,
        }
    }

    pub async fn process_request(
        &self,
        session_requestor: Arc<WebSocketRequestManager>,
        cmd: u16,
        content: String,
        source: HttpRequestSource,
        auth: Option<&InterfaceAuth>,
    ) -> BuckyResult<Option<String>> {
        match cmd {
            ROUTER_WS_HANDLER_CMD_ADD | ROUTER_WS_HANDLER_CMD_REMOVE => {
                if cmd == ROUTER_WS_HANDLER_CMD_ADD {
                    info!("recv add ws router handler request: {}", content);

                    let req = RouterWSAddHandlerParam::decode_string(&content)?;

                    if let Some(auth) = auth {
                        auth.check_option_dec(req.dec_id.as_ref(), &source)?;
                    }

                    let mut source = self.zone_manager.get_current_source_info(&req.dec_id).await?;
                    source.protocol = self.protocol;

                    self.on_add_handler_request(session_requestor, source, req)
                        .await
                        .map(|v| Some(v))
                } else {
                    info!("recv ws remove router handler request: {}", content);

                    let req = RouterWSRemoveHandlerParam::decode_string(&content)?;

                    if let Some(auth) = auth {
                        auth.check_option_dec(req.dec_id.as_ref(), &source)?;
                    }

                    self.on_remove_handler_request(req).map(|v| Some(v))
                }
            }

            _ => {
                let msg = format!(
                    "unknown ws router handler cmd: sid={}, cmd={}",
                    session_requestor.sid(),
                    cmd
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }

    async fn on_add_handler_request(
        &self,
        session_requestor: Arc<WebSocketRequestManager>,
        source: RequestSourceInfo,
        req: RouterWSAddHandlerParam,
    ) -> BuckyResult<String> {
        let resp = match self
            .processor
            .on_add_handler_request(session_requestor, source, &req)
            .await
        {
            Ok(_) => RouterWSHandlerResponse {
                err: BuckyErrorCode::Ok.into(),
                msg: None,
            },
            Err(e) => {
                error!("add ws router handler error! req={:?}, {}", req, e);

                RouterWSHandlerResponse {
                    err: e.code().into(),
                    msg: Some(e.msg().to_owned()),
                }
            }
        };

        Ok(resp.encode_string())
    }

    fn on_remove_handler_request(&self, req: RouterWSRemoveHandlerParam) -> BuckyResult<String> {
        let resp = match self.processor.on_remove_handler_request(req) {
            Ok(ret) => {
                if ret {
                    RouterWSHandlerResponse {
                        err: BuckyErrorCode::Ok.into(),
                        msg: None,
                    }
                } else {
                    RouterWSHandlerResponse {
                        err: BuckyErrorCode::NotFound.into(),
                        msg: None,
                    }
                }
            }
            Err(e) => RouterWSHandlerResponse {
                err: e.code().into(),
                msg: Some(e.msg().to_owned()),
            },
        };

        Ok(resp.encode_string())
    }
}
