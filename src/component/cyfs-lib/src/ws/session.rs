use super::packet::*;
use super::request::*;
use async_std::future::TimeoutError;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_debug::Mutex;

use async_std::channel::{Receiver, Sender};
use async_std::io::{Read, Write};
use async_tungstenite::{tungstenite::Message, WebSocketStream};
use futures::future::Either;
use futures_util::sink::*;
use futures_util::StreamExt;
use http_types::Url;
use std::marker::Send;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ping间隔
pub const WS_PING_INTERVAL_IN_SECS: Duration = Duration::from_secs(30);

// 连接上收不到任何消息的最大时长
#[cfg(debug_assertions)]
pub const WS_ALIVE_TIMEOUT_IN_SECS: Duration = Duration::from_secs(60 * 10);

#[cfg(not(debug_assertions))]
pub const WS_ALIVE_TIMEOUT_IN_SECS: Duration = Duration::from_secs(60 * 10);

pub struct WebSocketSession {
    sid: u32,

    // 连接信息
    conn_info: (SocketAddr, SocketAddr),
    source: String,

    // 消息发送端
    tx: Mutex<Option<Sender<Message>>>,

    handler: Box<dyn WebSocketRequestHandler>,
    requestor: Arc<WebSocketRequestManager>,
}

impl Drop for WebSocketSession {
    fn drop(&mut self) {
        warn!("ws session dropped! sid={}", self.sid);
    }
}

impl WebSocketSession {
    pub fn new(
        sid: u32,
        source: String,
        conn_info: (SocketAddr, SocketAddr),
        handler: Box<dyn WebSocketRequestHandler>,
    ) -> Self {
        info!("new ws session: sid={}, source={}", sid, source);

        Self {
            sid,
            conn_info,
            source,
            tx: Mutex::new(None),
            handler: handler.clone_handler(),
            requestor: Arc::new(WebSocketRequestManager::new(handler)),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.requestor.is_session_valid()
    }

    pub fn requestor(&self) -> &Arc<WebSocketRequestManager> {
        &self.requestor
    }

    pub fn sid(&self) -> u32 {
        self.sid
    }

    pub fn conn_info(&self) -> &(SocketAddr, SocketAddr) {
        &self.conn_info
    }

    pub async fn post_msg(&self, msg: Vec<u8>) -> BuckyResult<()> {
        let tx = self.tx.lock().unwrap().clone();
        if let Some(tx) = tx {
            let msg = Message::binary(msg);
            if let Err(e) = tx.send(msg).await {
                warn!("session tx already closed! sid={}, {}", self.sid, e);
                Err(BuckyError::from(BuckyErrorCode::NotConnected))
            } else {
                Ok(())
            }
        } else {
            // session已经结束，直接忽略
            warn!("session tx not exists! sid={}", self.sid);
            Err(BuckyError::from(BuckyErrorCode::NotConnected))
        }
    }

    pub async fn run_client<S>(session: Arc<Self>, service_url: &Url, stream: S) -> BuckyResult<()>
    where
        S: Read + Write + Unpin + Send + 'static,
    {
        let (stream, _) = async_tungstenite::client_async(service_url, stream)
            .await
            .map_err(|e| {
                let msg = format!("ws connect error: service_url={}, err={}", service_url, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::Unknown, msg)
            })?;

        Self::run(session, stream, false).await
    }

    pub async fn run_server<S>(session: Arc<Self>, stream: S) -> BuckyResult<()>
    where
        S: Read + Write + Unpin + Send + 'static,
    {
        let stream = async_tungstenite::accept_async(stream).await.map_err(|e| {
            let msg = format!("ws accept error: err={}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::Unknown, msg)
        })?;

        Self::run(session, stream, true).await
    }

    async fn run<S>(
        session: Arc<Self>,
        stream: WebSocketStream<S>,
        as_server: bool,
    ) -> BuckyResult<()>
    where
        S: Read + Write + Unpin + Send + 'static,
    {
        let (tx, rx) = async_std::channel::bounded::<Message>(1024);

        // 保存sender
        {
            let mut current = session.tx.lock().unwrap();
            assert!(current.is_none());
            *current = Some(tx.clone());
        }

        // 初始化请求管理器
        session.requestor.bind_session(session.clone());

        // 正式通知session启动了
        session.handler.on_session_begin(&session).await;

        let ret = Self::run_loop(session.clone(), stream, rx, as_server).await;

        session.handler.on_session_end(&session).await;

        // 通知session结束
        session.requestor.unbind_session();

        // 终止发送
        {
            let tx = session.tx.lock().unwrap().take();
            assert!(tx.is_some());
        }

        ret
    }

    async fn run_loop<S>(
        session: Arc<Self>,
        stream: WebSocketStream<S>,
        rx: Receiver<Message>,
        with_ping: bool,
    ) -> BuckyResult<()>
    where
        S: Read + Write + Unpin + Send + 'static,
    {
        let (mut outgoing, mut incoming) = stream.split();

        // 记录最后一次活动时间
        let mut last_alive = Instant::now();

        let ret = loop {
            // trace!("try recv from ws session: {}", session.sid());

            let send_recv = futures::future::select(incoming.next(), rx.recv());
            let ret = async_std::future::timeout(WS_PING_INTERVAL_IN_SECS, send_recv).await;

            // trace!("recv sth. from ws session: {}, ret={:?}", session.sid(), ret);

            match ret {
                Err(TimeoutError { .. }) => {
                    if with_ping {
                        let msg = Message::Ping(Vec::new());
                        if let Err(e) = outgoing.send(msg).await {
                            let msg = format!(
                                "ws send msg error: sid={}, err={}",
                                session.sid(),
                                e
                            );
                            warn!("{}", msg);

                            break Err(BuckyError::new(
                                BuckyErrorCode::ConnectionAborted,
                                msg,
                            ));
                        }
                    }

                    // 检查连接是否还在活跃
                    let now = Instant::now();
                    if now - last_alive >= WS_ALIVE_TIMEOUT_IN_SECS {
                        let msg = format!("ws session alive timeout: sid={}", session.sid());
                        error!("{}", msg);

                        break Err(BuckyError::new(BuckyErrorCode::Timeout, msg));
                    }

                    continue;
                }
                Ok(ret) => {
                    match ret {
                        Either::Left((ret, _fut)) => {
                            if ret.is_none() {
                                info!(
                                    "ws recv complete, sid={}, source={}",
                                    session.sid(),
                                    session.source
                                );
                                break Ok(());
                            }

                            match ret.unwrap() {
                                Ok(msg) => {
                                    if msg.is_close() {
                                        info!(
                                            "ws rx closed msg: sid={}, source={}",
                                            session.sid(),
                                            session.source
                                        );
                                        break Ok(());
                                    }

                                    // 收到了有效消息，需要更新最后活跃时刻
                                    last_alive = Instant::now();

                                    // 如果收到ping后，那么需要答复pong
                                    if msg.is_ping() {
                                        // 会自动发送pong
                                        /*
                                        trace!(
                                            "ws recv ping: sid={}, is_server={}",
                                            session.sid(),
                                            as_server
                                        );
                                        */
                                        continue;
                                    } else if msg.is_pong() {
                                        /*
                                        trace!(
                                            "ws recv pong: sid={}, is_server={}",
                                            session.sid(),
                                            as_server
                                        );
                                        */
                                        continue;
                                    }

                                    async_std::task::spawn(Self::process_msg(
                                        session.requestor.clone(),
                                        msg,
                                    ));
                                }

                                Err(e) => {
                                    let msg =
                                        format!("ws recv error: sid={}, err={}", session.sid(), e);
                                    warn!("{}", msg);

                                    break Err(BuckyError::new(
                                        BuckyErrorCode::ConnectionAborted,
                                        msg,
                                    ));
                                }
                            }
                        }
                        Either::Right((ret, _fut)) => match ret {
                            Ok(msg) => {
                                if let Err(e) = outgoing.send(msg).await {
                                    let msg = format!(
                                        "ws send msg error: sid={}, err={}",
                                        session.sid(),
                                        e
                                    );
                                    warn!("{}", msg);

                                    break Err(BuckyError::new(
                                        BuckyErrorCode::ConnectionAborted,
                                        msg,
                                    ));
                                }
                            }
                            Err(e) => {
                                info!("ws send msg stopped: {}", e);
                                break Ok(());
                            }
                        },
                    }
                }
            }
        };

        ret
    }

    async fn process_msg(requestor: Arc<WebSocketRequestManager>, msg: Message) -> BuckyResult<()> {
        let data = msg.into_data();
        let packet = WSPacket::decode(data)?;

        match WebSocketRequestManager::on_msg(requestor, packet).await {
            Ok(_) => {
                // 处理消息成功了
            }
            Err(e) => {
                error!("process ws request error: {}", e);

                /*
                // 处理消息失败，不需要终止当前session
                // 只要包格式正确,session就可以继续使用
                *has_err.lock().unwrap() = Some(e);

                let mut abort_state = abort_state.lock().unwrap();
                abort_state.is_abort = true;
                if let Some(abort_handle) = abort_state.handle.take() {
                    warn!("will abort ws session: {}", requestor.sid());
                    abort_handle.abort();
                }
                */
            }
        }

        Ok(())
    }
}
