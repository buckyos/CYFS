use super::packet::*;
use super::request::*;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_debug::Mutex;

use async_std::channel::{Receiver, Sender};
use async_std::io::{Read, Write};
use async_tungstenite::{tungstenite::Message, WebSocketStream};
use futures::future::{AbortHandle, Abortable};
use futures_util::StreamExt;
use futures_util::{sink::*, stream::SplitSink};
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

struct WebSocketSender<S>
where
    S: Read + Write + Unpin + Send + 'static,
{
    rx: Receiver<Message>,
    outgoing: SplitSink<WebSocketStream<S>, Message>,
}

impl<S> WebSocketSender<S>
where
    S: Read + Write + Unpin + Send + 'static,
{
    fn start_run(self) -> AbortHandle {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let fut = Abortable::new(self.run(), abort_registration);

        async_std::task::spawn(async move {
            let _ret = fut.await;
        });

        abort_handle
    }

    fn start_run_with_ping(self) -> AbortHandle {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let fut = Abortable::new(self.run_with_ping(), abort_registration);

        async_std::task::spawn(async move {
            let _ret = fut.await;
        });

        abort_handle
    }

    async fn run(mut self) {
        loop {
            match self.rx.recv().await {
                Ok(msg) => {
                    if let Err(e) = self.outgoing.send(msg).await {
                        error!("ws send msg failed, now will stop: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    info!("ws send msg stopped: {}", e);
                    break;
                }
            }
        }
    }

    async fn run_with_ping(mut self) {
        loop {
            let ret = async_std::future::timeout(WS_PING_INTERVAL_IN_SECS, self.rx.recv()).await;

            match ret {
                Ok(ret) => match ret {
                    Ok(msg) => {
                        if let Err(e) = self.outgoing.send(msg).await {
                            error!("ws send msg failed, now will stop: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        info!("ws send msg stopped: {}", e);
                        break;
                    }
                },

                Err(_) => {
                    let msg = Message::Ping(Vec::new());
                    if let Err(e) = self.outgoing.send(msg).await {
                        error!("ws send ping failed, now will stop: {}", e);
                        break;
                    }
                }
            }
        }
    }
}

pub struct WebSocketSession {
    sid: u32,

    // 连接信息
    conn_info: (SocketAddr, SocketAddr),
    source: String,

    // 消息发送端
    tx: Arc<Mutex<Option<Sender<Message>>>>,

    handler: Box<dyn WebSocketRequestHandler>,
    requestor: Arc<WebSocketRequestManager>,
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
            tx: Arc::new(Mutex::new(None)),
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
        let (outgoing, mut incoming) = stream.split();

        // 保存sender
        {
            let mut current = session.tx.lock().unwrap();
            assert!(current.is_none());
            *current = Some(tx.clone());
        }

        // 初始化请求管理器
        session.requestor.bind_session(session.clone());

        // ws的消息发送器
        let sender = WebSocketSender { rx, outgoing };

        let sender_canceller = if as_server {
            // server端需要开启ping
            sender.start_run_with_ping()
        } else {
            // client端不需要开启ping，收到ping后需要应答pong
            sender.start_run()
        };

        // 正式通知session启动了
        session.handler.on_session_begin(&session).await;

        // 消息的流式解析器
        let mut parser = WSPacketParser::new();

        // 记录最后一次活动时间
        let mut last_alive = Instant::now();

        let ret = loop {
  
            // trace!("try recv from ws session: {}", session.sid());

            let ret = async_std::future::timeout(WS_PING_INTERVAL_IN_SECS, incoming.next()).await;

            // trace!("recv sth. from ws session: {}, ret={:?}", session.sid(), ret);

            // 判断是不是超时
            if ret.is_err() {
                // 检查连接是否还在活跃
                let now = Instant::now();
                if now - last_alive >= WS_ALIVE_TIMEOUT_IN_SECS {
                    let msg = format!("ws session alive timeout: sid={}", session.sid());
                    error!("{}", msg);

                    break Err(BuckyError::new(BuckyErrorCode::Timeout, msg));
                }

                continue;
            }

            let ret = ret.unwrap();
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

                    let mut data = msg.into_data();
                    if let Err(e) = parser.push(&mut data) {
                        error!("parse ws packet error: {}", e);
                        break Err(e);
                    }

                    while let Some(packet) = parser.next_packet() {
                        let requestor = session.requestor.clone();
                        // let abort_state = abort_state.clone();
                        // let has_err = has_err.clone();

                        async_std::task::spawn(async move {
                            match WebSocketRequestManager::on_msg(requestor.clone(), packet).await {
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
                        });
                    }
                }

                Err(e) => {
                    let msg = format!("ws recv error: sid={}, err={}", session.sid(), e);
                    warn!("{}", msg);

                    break Err(BuckyError::new(BuckyErrorCode::ConnectionAborted, msg));
                }
            }
        };

        session.handler.on_session_end(&session).await;

        // 通知session结束
        session.requestor.unbind_session();

        // 终止发送循环
        sender_canceller.abort();

        // 终止发送
        {
            let tx = session.tx.lock().unwrap().take();
            assert!(tx.is_some());
        }

        ret
    }
}
