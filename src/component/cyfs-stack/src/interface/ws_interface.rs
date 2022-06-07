use crate::events::*;
use crate::router_handler::*;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_lib::*;
use super::http_ws_listener::ObjectHttpWSService;


use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
struct WebSocketRequestInnerHandler {
    http_ws_service: ObjectHttpWSService,
    router_handlers_handler: Arc<RouterHandlerWebSocketHandler>,
    router_events_handler: Arc<RouterEventWebSocketHandler>,
}

#[async_trait]
impl WebSocketRequestHandler for WebSocketRequestInnerHandler {
    async fn on_request(
        &self,
        session_requestor: Arc<WebSocketRequestManager>,
        cmd: u16,
        content: Vec<u8>,
    ) -> BuckyResult<Option<Vec<u8>>> {
        match cmd {
            HTTP_CMD_REQUEST => {
                self.http_ws_service.process_request(session_requestor, content).await.map(|resp| Some(resp))
            }
            _ => {
                self.process_string_request(session_requestor, cmd, content)
                    .await
            }
        }
    }

    async fn on_string_request(
        &self,
        session_requestor: Arc<WebSocketRequestManager>,
        cmd: u16,
        content: String,
    ) -> BuckyResult<Option<String>> {
        match cmd {
            ROUTER_WS_HANDLER_CMD_ADD | ROUTER_WS_HANDLER_CMD_REMOVE => {
                self.router_handlers_handler
                    .process_request(session_requestor, cmd, content)
                    .await
            }
            ROUTER_WS_EVENT_CMD_ADD | ROUTER_WS_EVENT_CMD_REMOVE => {
                self.router_events_handler
                    .process_request(session_requestor, cmd, content)
                    .await
            }
            _ => {
                let msg = format!(
                    "unknown ws router-handler/event cmd: sid={}, cmd={}",
                    session_requestor.sid(),
                    cmd
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }

    async fn on_session_begin(&self, session: &Arc<WebSocketSession>) {
        info!("ws event new session: sid={}", session.sid());
    }

    async fn on_session_end(&self, session: &Arc<WebSocketSession>) {
        info!("ws event end session: sid={}", session.sid());
    }

    fn clone_handler(&self) -> Box<dyn WebSocketRequestHandler> {
        Box::new(self.clone())
    }
}

pub(super) struct WebSocketEventInterface {
    server: WebSocketServer,
}

impl WebSocketEventInterface {
    pub fn new(
        http_ws_service: ObjectHttpWSService,
        router_handlers_manager: RouterHandlersManager,
        router_events_manager: RouterEventsManager,
        addr: SocketAddr,
    ) -> Self {
        let router_handlers_handler =
            RouterHandlerWebSocketHandler::new(NONProtocol::HttpLocal, router_handlers_manager);
        let router_events_handler =
            RouterEventWebSocketHandler::new(NONProtocol::HttpLocal, router_events_manager);

        let handler = WebSocketRequestInnerHandler {
            http_ws_service,
            router_handlers_handler: Arc::new(router_handlers_handler),
            router_events_handler: Arc::new(router_events_handler),
        };

        let server = WebSocketServer::new(addr, Box::new(handler));
        Self { server }
    }

    pub fn get_ws_event_listener(&self) -> SocketAddr {
        self.server.get_addr()
    }

    pub async fn start(&self) -> BuckyResult<()> {
        self.server.start().await
    }

    pub async fn stop(&self) {
        self.server.stop().await
    }

    pub async fn restart(&self) -> BuckyResult<()> {
        self.server.stop().await;
        self.server.start().await
    }
}
