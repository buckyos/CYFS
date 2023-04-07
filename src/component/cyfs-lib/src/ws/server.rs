use super::request::*;
use super::session_manager::*;
use crate::base::{BaseTcpListener, BaseTcpListenerHandler};
use cyfs_base::*;

use async_std::net::TcpStream;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
pub struct WebSocketServer {
    tcp_server: BaseTcpListener,

    session_manager: WebSocketSessionManager,
}

impl WebSocketServer {
    pub fn new(addr: SocketAddr, handler: Box<dyn WebSocketRequestHandler>) -> Self {
        let ret = Self {
            tcp_server: BaseTcpListener::new(addr),
            session_manager: WebSocketSessionManager::new(handler),
        };

        let tcp_handler = Arc::new(Box::new(ret.clone()) as Box<dyn BaseTcpListenerHandler>);
        ret.tcp_server.bind_handler(tcp_handler);

        ret
    }

    pub fn get_addr(&self) -> SocketAddr {
        self.tcp_server.get_addr()
    }

    pub async fn start(&self) -> BuckyResult<()> {
        match self.tcp_server.start().await {
            Ok(_) => {
                info!("ws server start success! addr={}", self.get_addr());
                Ok(())
            }
            Err(e) => {
                info!("ws server start failed! addr={}, {}", self.get_addr(), e);
                Err(e)
            }
        }
    }

    pub async fn stop(&self) {
        self.tcp_server.stop().await
    }
}

#[async_trait::async_trait]
impl BaseTcpListenerHandler for WebSocketServer {
    async fn on_accept(&self, tcp_stream: TcpStream) -> BuckyResult<()> {
        let local_addr = tcp_stream.local_addr().map_err(|e| {
            let msg = format!("get local_addr from tcp stream but failed! {}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::NotConnected, msg)
        })?;

        let peer_addr = tcp_stream.peer_addr().map_err(|e| {
            let msg = format!("get peer_addr from tcp stream but failed! {}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::NotConnected, msg)
        })?;

        let conn_info = (local_addr, peer_addr);

        self.session_manager
            .run_server_session(conn_info.1.to_string(), conn_info, tcp_stream);
        Ok(())
    }
}
