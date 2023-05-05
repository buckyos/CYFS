use super::request::*;
use super::session::*;
use super::session_manager::*;
use cyfs_base::*;

use async_std::net::TcpStream;
use cyfs_debug::Mutex;
use futures::future::{AbortHandle, Abortable, Aborted};
use http_types::Url;
use std::net::SocketAddr;
use std::sync::Arc;

// connect的重试间隔
const WS_CONNECT_RETRY_MIN_INTERVAL_SECS: u64 = 2;
const WS_CONNECT_RETRY_MAX_INTERVAL_SECS: u64 = 60;

struct WebSocketClientHandles {
    // 用以唤醒重试等待
    waker: Option<AbortHandle>,

    // 取消运行
    running_task: Option<async_std::task::JoinHandle<()>>,
}

impl Default for WebSocketClientHandles {
    fn default() -> Self {
        Self {
            waker: None,
            running_task: None,
        }
    }
}

struct WebSocketClientSessionState {
    session: Option<Arc<WebSocketSession>>,
    stopped: bool,
}

#[derive(Clone)]
pub struct WebSocketClient {
    service_url: Url,
    service_addr: SocketAddr,

    session_manager: WebSocketSessionManager,

    // 用以唤醒重试等待
    handles: Arc<Mutex<WebSocketClientHandles>>,

    session: Arc<Mutex<WebSocketClientSessionState>>,
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
            handles: Arc::new(Mutex::new(WebSocketClientHandles::default())),
            session: Arc::new(Mutex::new(WebSocketClientSessionState {
                session: None,
                stopped: false,
            })),
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
        let task = async_std::task::spawn(this.run());

        let mut handles = self.handles.lock().unwrap();
        assert!(handles.running_task.is_none());
        handles.running_task = Some(task);
    }

    async fn run(self) {
        let mut retry_interval = WS_CONNECT_RETRY_MIN_INTERVAL_SECS;

        loop {
            info!("ws will connect to {}", self.service_url);

            match self.run_once().await {
                Ok(_) => {
                    warn!("ws session complete");
                    retry_interval = WS_CONNECT_RETRY_MIN_INTERVAL_SECS;
                }
                Err(e) => {
                    if e.code() == BuckyErrorCode::Aborted {
                        break;
                    }
                    error!("ws session complete with error: {}", e);
                }
            };

            let (abort_handle, abort_registration) = AbortHandle::new_pair();
            self.handles.lock().unwrap().waker = Some(abort_handle);
            let future = Abortable::new(
                async_std::task::sleep(std::time::Duration::from_secs(retry_interval)),
                abort_registration,
            );

            match future.await {
                Ok(_) => {
                    retry_interval *= 2;
                    if retry_interval >= WS_CONNECT_RETRY_MAX_INTERVAL_SECS {
                        retry_interval = WS_CONNECT_RETRY_MAX_INTERVAL_SECS;
                    }
                }
                Err(Aborted { .. }) => {
                    retry_interval = WS_CONNECT_RETRY_MIN_INTERVAL_SECS;
                }
            };
        }

        info!("ws client stopped! url={}", self.service_url);
    }

    // 立即发起一次重试
    pub fn retry(&self) {
        if let Some(waker) = self.handles.lock().unwrap().waker.take() {
            waker.abort();
        }
    }

    pub async fn run_once(&self) -> BuckyResult<()> {
        let tcp_stream = TcpStream::connect(self.service_addr).await.map_err(|e| {
            let msg = format!("ws connect to {} error: {}", self.service_addr, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        let conn_info = (
            tcp_stream.local_addr().unwrap(),
            tcp_stream.peer_addr().unwrap(),
        );

        let session = self
            .session_manager
            .new_session(&self.service_url, conn_info)?;

        let stopped = {
            let mut state = self.session.lock().unwrap();
            assert!(state.session.is_none());
            if !state.stopped {
                state.session = Some(session.clone());
            } else {
                warn!(
                    "ws client run session but already stopped! sid={}",
                    session.sid()
                );
            }

            state.stopped
        };

        let ret = if !stopped {
            let ret = self
                .session_manager
                .run_client_session(&self.service_url, session, tcp_stream)
                .await;

            self.session.lock().unwrap().session.take();

            ret
        } else {
            Err(BuckyError::from(BuckyErrorCode::Aborted))
        };

        ret
    }

    pub async fn stop(&self) {
        let session = {
            let mut state = self.session.lock().unwrap();
            state.stopped = true;
            state.session.take()
        };

        if let Some(session) = session {
            session.stop();
        }

        let task = self.handles.lock().unwrap().running_task.take();
        if let Some(task) = task {
            task.await;
        }

        self.session_manager.stop();
    }
}
