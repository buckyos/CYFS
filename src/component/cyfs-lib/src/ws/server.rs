use super::request::*;
use super::session_manager::*;
use crate::base::{BaseTcpListener, BaseTcpListenerHandler};
use cyfs_base::*;
use super::check::{WebSocketSessionCheckerRef, WebSocketPeekStream};

use async_std::net::TcpStream;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
pub struct WebSocketServer {
    tcp_server: BaseTcpListener,

    session_manager: WebSocketSessionManager,

    checker: Option<WebSocketSessionCheckerRef>,
}

impl WebSocketServer {
    pub fn new(addr: SocketAddr, handler: Box<dyn WebSocketRequestHandler>, checker: Option<WebSocketSessionCheckerRef>) -> Self {
        let ret = Self {
            tcp_server: BaseTcpListener::new(addr),
            session_manager: WebSocketSessionManager::new(handler),
            checker,
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
        let conn_info = (
            tcp_stream.local_addr().unwrap(),
            tcp_stream.peer_addr().unwrap(),
        );

        // debug!("new ws connect stream: {:?}", conn_info);

        if let Some(checker) = &self.checker {
            let s = WebSocketPeekStream::new(tcp_stream.clone()).await?;
            let ret = async_h1::server::decode(s).await.map_err(|e| {
                let msg = format!("decode request from stream error! {:?}, {}", conn_info, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            if let Some((req, _reader)) = ret {
                debug!("will check ws session request: conn={:?}, {:?}", conn_info, req);
                checker.check(req).await?;
            }
        }

        self.session_manager
            .run_server_session(tcp_stream.peer_addr().unwrap().to_string(), conn_info, tcp_stream);
        Ok(())
    }
}