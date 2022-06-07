use super::super::RouterEventsManager;
use super::processor::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct RouterEventWebSocketHandler {
    protocol: NONProtocol,

    processor: RouterEventWSProcessor,
}

impl Clone for RouterEventWebSocketHandler {
    fn clone(&self) -> Self {
        Self {
            protocol: self.protocol.clone(),
            processor: self.processor.clone(),
        }
    }
}

impl RouterEventWebSocketHandler {
    pub fn new(protocol: NONProtocol, manager: RouterEventsManager) -> Self {
        let processor = RouterEventWSProcessor::new(manager);
        Self {
            protocol,
            processor,
        }
    }

    pub async fn process_request(
        &self,
        session_requestor: Arc<WebSocketRequestManager>,
        cmd: u16,
        content: String,
    ) -> BuckyResult<Option<String>> {
        match cmd {
            ROUTER_WS_EVENT_CMD_ADD | ROUTER_WS_EVENT_CMD_REMOVE => {
                if cmd == ROUTER_WS_EVENT_CMD_ADD {
                    info!("recv add ws router event request: {}", content);

                    let req = RouterWSAddEventParam::decode_string(&content)?;
                    self.on_add_event_request(session_requestor, req)
                        .await
                        .map(|v| Some(v))
                } else {
                    info!("recv ws remove router event request: {}", content);

                    let req = RouterWSRemoveEventParam::decode_string(&content)?;
                    self.on_remove_event_request(req).map(|v| Some(v))
                }
            }

            _ => {
                let msg = format!(
                    "unknown ws router event cmd: sid={}, cmd={}",
                    session_requestor.sid(),
                    cmd
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }

    async fn on_add_event_request(
        &self,
        session_requestor: Arc<WebSocketRequestManager>,
        req: RouterWSAddEventParam,
    ) -> BuckyResult<String> {
        let resp = match self
            .processor
            .on_add_event_request(session_requestor, &req)
            .await
        {
            Ok(_) => RouterWSEventResponse {
                err: BuckyErrorCode::Ok.into(),
                msg: None,
            },
            Err(e) => {
                error!("add ws router event error! req={:?}, {}", req, e);

                RouterWSEventResponse {
                    err: e.code().into(),
                    msg: Some(e.msg().to_owned()),
                }
            }
        };

        Ok(resp.encode_string())
    }

    fn on_remove_event_request(&self, req: RouterWSRemoveEventParam) -> BuckyResult<String> {
        let resp = match self.processor.on_remove_event_request(req) {
            Ok(ret) => {
                if ret {
                    RouterWSEventResponse {
                        err: BuckyErrorCode::Ok.into(),
                        msg: None,
                    }
                } else {
                    RouterWSEventResponse {
                        err: BuckyErrorCode::NotFound.into(),
                        msg: None,
                    }
                }
            }
            Err(e) => RouterWSEventResponse {
                err: e.code().into(),
                msg: Some(e.msg().to_owned()),
            },
        };

        Ok(resp.encode_string())
    }
}
