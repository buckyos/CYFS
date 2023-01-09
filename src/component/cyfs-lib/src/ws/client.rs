use super::request::*;
use super::session::*;
use super::session_manager::*;
use cyfs_base::{BuckyError, BuckyResult};

use async_std::net::TcpStream;
use http_types::Url;
use std::net::SocketAddr;
use std::sync::Arc;
use futures::future::{AbortHandle, Abortable};
use cyfs_debug::Mutex;

// connect的重试间隔
const WS_CONNECT_RETRY_MIN_INTERVAL_SECS: u64 = 2;
const WS_CONNECT_RETRY_MAX_INTERVAL_SECS: u64 = 60;

#[derive(Clone)]
pub struct WebSocketClient {
    service_url: Url,
    service_addr: SocketAddr,

    session_manager: WebSocketSessionManager,

    // 用以唤醒重试等待
    waker: Arc<Mutex<Option<AbortHandle>>>,
}

impl WebSocketClient {
    pub fn new(service_url: Url, handler: Box<dyn WebSocketRequestHandler>) -> Self {
        let service_addr = format!(
            "{}:{}",
            service_url.host().unwrap(),
            service_url.port().unwrap()
        )
        .parse()
        .unwrap();

        Self {
            service_url,
            service_addr,
            session_manager: WebSocketSessionManager::new(handler),
            waker: Arc::new(Mutex::new(None)),
        }
    }

    pub fn service_addr(&self) -> &SocketAddr {
        &self.service_addr
    }

    // 随机选择一个session
    pub fn select_session(&self) -> Option<Arc<WebSocketSession>> {
        self.session_manager.select_session()
    }

    pub fn start(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            this.run().await;
        });
    }

    pub async fn run(self) {
        let mut retry_interval = WS_CONNECT_RETRY_MIN_INTERVAL_SECS;

        loop {
            info!("will ws connect to {}", self.service_url);

            match self.run_once().await {
                Ok(_) => {
                    warn!("ws session complete");
                    retry_interval = WS_CONNECT_RETRY_MIN_INTERVAL_SECS;
                }
                Err(e) => {
                    error!("ws session complete with error: {}", e);
                }
            };

            let (abort_handle, abort_registration) = AbortHandle::new_pair();
            *self.waker.lock().unwrap() = Some(abort_handle);
            let future = Abortable::new(
                async_std::task::sleep(std::time::Duration::from_secs(retry_interval)), 
                abort_registration
            );

            match future.await {
                Ok(_) => {
                    retry_interval *= 2;
                    if retry_interval >= WS_CONNECT_RETRY_MAX_INTERVAL_SECS {
                        retry_interval = WS_CONNECT_RETRY_MAX_INTERVAL_SECS;
                    }
                }
                Err(futures::future::Aborted { .. }) => {
                    retry_interval = WS_CONNECT_RETRY_MIN_INTERVAL_SECS;
                }
            };
        }
    }

    // 立即发起一次重试
    pub fn retry(&self) {
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.abort();
        }
    }

    pub async fn run_once(&self) -> BuckyResult<()> {
        let tcp_stream = TcpStream::connect(self.service_addr).await.map_err(|e| {
            let msg = format!("ws connect to {} error: {}", self.service_addr, e);
            error!("{}", msg);

            BuckyError::from(e)
        })?;

        let conn_info = (
            tcp_stream.local_addr().unwrap(),
            tcp_stream.peer_addr().unwrap(),
        );

        self.session_manager
            .run_client_session(&self.service_url, conn_info, tcp_stream)
            .await
    }
}
