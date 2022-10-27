use super::protocol::*;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, Device, DeviceId, Endpoint, NamedObject};
use cyfs_bdt::{BuildTunnelParams, StackGuard};
use http_types::{Request, Response};

use async_std::net::{SocketAddr, TcpStream};
use async_trait::async_trait;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

pub enum HttpRequestConnectionInfo {
    None,
    Tcp((SocketAddr, SocketAddr)),
    Bdt((Endpoint, Endpoint)),
}

#[async_trait]
pub trait HttpRequestor: Send + Sync {
    fn remote_addr(&self) -> String;
    fn remote_device(&self) -> Option<DeviceId>;

    async fn request(&self, req: Request) -> BuckyResult<Response> {
        self.request_ext(&mut Some(req), None).await
    }

    async fn request_with_conn_info(
        &self,
        req: Request,
        conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response> {
        self.request_ext(&mut Some(req), conn_info).await
    }

    async fn request_with_conn_info_timeout(
        &self,
        req: Request,
        conn_info: Option<&mut HttpRequestConnectionInfo>,
        dur: Duration,
    ) -> BuckyResult<Response> {
        self.request_ext_timeout(&mut Some(req), dur, conn_info)
            .await
    }

    async fn request_timeout(&self, req: Request, dur: Duration) -> BuckyResult<Response> {
        self.request_ext_timeout(&mut Some(req), dur, None).await
    }

    async fn request_ext_timeout(
        &self,
        req: &mut Option<Request>,
        dur: Duration,
        conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response> {
        match async_std::future::timeout(dur, self.request_ext(req, conn_info)).await {
            Ok(ret) => ret,
            Err(async_std::future::TimeoutError { .. }) => {
                let msg = format!("request timeout, remote={}", self.remote_addr(),);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::Timeout, msg))
            }
        }
    }

    async fn request_ext(
        &self,
        req: &mut Option<Request>,
        conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response>;

    fn clone_requestor(&self) -> Box<dyn HttpRequestor>;
}

pub type HttpRequestorRef = Arc<Box<dyn HttpRequestor>>;

#[derive(Clone)]
pub struct TcpHttpRequestor {
    service_addr: SocketAddr,
}

impl TcpHttpRequestor {
    pub fn new(service_addr: &str) -> Self {
        let service_addr = SocketAddr::from_str(&service_addr).unwrap();
        Self { service_addr }
    }
}

#[async_trait]
impl HttpRequestor for TcpHttpRequestor {
    async fn request_ext(
        &self,
        req: &mut Option<Request>,
        conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response> {
        debug!(
            "will http-local request to {}, url={}",
            self.remote_addr(),
            req.as_ref().unwrap().url()
        );

        let begin = std::time::Instant::now();
        let tcp_stream = TcpStream::connect(self.service_addr).await.map_err(|e| {
            let msg = format!(
                "tcp connect to {} error! during={}ms, {}",
                self.service_addr,
                begin.elapsed().as_millis(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        info!(
            "tcp connect to {} success, during={}ms",
            self.remote_addr(),
            begin.elapsed().as_millis(),
        );

        if let Some(conn_info) = conn_info {
            *conn_info = HttpRequestConnectionInfo::Tcp((
                tcp_stream.local_addr().unwrap(),
                tcp_stream.peer_addr().unwrap(),
            ));
        }

        match async_h1::connect(tcp_stream, req.take().unwrap()).await {
            Ok(resp) => {
                info!(
                    "http-tcp request to {} success! during={}ms",
                    self.remote_addr(),
                    begin.elapsed().as_millis()
                );
                Ok(resp)
            }
            Err(e) => {
                let msg = format!(
                    "http-tcp request to {} failed! during={}ms, {}",
                    self.remote_addr(),
                    begin.elapsed().as_millis(),
                    e,
                );
                error!("{}", msg);
                Err(BuckyError::from(msg))
            }
        }
    }

    fn remote_addr(&self) -> String {
        self.service_addr.to_string()
    }

    fn remote_device(&self) -> Option<DeviceId> {
        None
    }

    fn clone_requestor(&self) -> Box<dyn HttpRequestor> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub struct BdtHttpRequestor {
    bdt_stack: StackGuard,
    device_id: DeviceId,
    device: Device,
    vport: u16,
}

impl BdtHttpRequestor {
    pub fn new(bdt_stack: StackGuard, device: Device, vport: u16) -> Self {
        Self {
            bdt_stack,
            device_id: device.desc().device_id(),
            device,
            vport,
        }
    }
}

#[async_trait]
impl HttpRequestor for BdtHttpRequestor {
    async fn request_ext(
        &self,
        req: &mut Option<Request>,
        conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response> {
        debug!(
            "will create bdt stream connection to {}",
            self.remote_addr()
        );

        // 如果device对象里面没有指定sn_list，那么使用默认值
        let mut sn_list = self.device.connect_info().sn_list().clone();
        if sn_list.is_empty() {
            sn_list = vec![cyfs_util::get_default_sn_desc().desc().device_id()];
        }

        let begin = std::time::Instant::now();
        let build_params = BuildTunnelParams {
            remote_const: self.device.desc().clone(),
            remote_sn: sn_list,
            remote_desc: None,
        };

        let bdt_stream = self
            .bdt_stack
            .stream_manager()
            .connect(self.vport, Vec::new(), build_params)
            .await
            .map_err(|e| {
                let msg = format!(
                    "connect to {} failed! during={}ms, {}",
                    self.remote_addr(),
                    begin.elapsed().as_millis(),
                    e
                );
                warn!("{}", msg);
                BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
            })?;

        if let Some(conn_info) = conn_info {
            *conn_info = HttpRequestConnectionInfo::Bdt((
                bdt_stream.local_ep().unwrap(),
                bdt_stream.remote_ep().unwrap(),
            ));
        }

        let seq = bdt_stream.sequence();
        info!(
            "bdt connect to {} success, seq={:?}, during={}ms",
            self.remote_addr(),
            seq,
            begin.elapsed().as_millis(),
        );
        // bdt_stream.display_ref_count();

        match async_h1::connect(bdt_stream, req.take().unwrap()).await {
            Ok(resp) => {
                info!(
                    "http-bdt request to {} success! during={}ms, seq={:?}",
                    self.remote_addr(),
                    begin.elapsed().as_millis(),
                    seq,
                );
                Ok(resp)
            }
            Err(e) => {
                let msg = format!(
                    "http-bdt request to {} failed! during={}ms, seq={:?}, {}",
                    self.remote_addr(),
                    begin.elapsed().as_millis(),
                    seq,
                    e,
                );
                error!("{}", msg);
                Err(BuckyError::from(msg))
            }
        }
    }

    fn remote_addr(&self) -> String {
        format!("{}:{}", self.device_id, self.vport)
    }

    fn remote_device(&self) -> Option<DeviceId> {
        Some(self.device_id.clone())
    }

    fn clone_requestor(&self) -> Box<dyn HttpRequestor> {
        Box::new(self.clone())
    }
}

use crate::ws::*;
use async_std::io::ReadExt;
use http_types::Url;

#[derive(Clone)]
struct WSHttpRequestorHandler {}
impl WSHttpRequestorHandler {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl WebSocketRequestHandler for WSHttpRequestorHandler {
    async fn on_request(
        &self,
        _requestor: Arc<WebSocketRequestManager>,
        _cmd: u16,
        _content: Vec<u8>,
    ) -> BuckyResult<Option<Vec<u8>>> {
        unreachable!();
    }

    async fn on_session_begin(&self, _session: &Arc<WebSocketSession>) {}

    async fn on_session_end(&self, _session: &Arc<WebSocketSession>) {}

    fn clone_handler(&self) -> Box<dyn WebSocketRequestHandler> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub struct WSHttpRequestor {
    client: WebSocketClient,
}

impl WSHttpRequestor {
    pub fn new(service_url: Url) -> Self {
        let handler = Box::new(WSHttpRequestorHandler::new());
        let client = WebSocketClient::new(service_url, handler);
        client.start();

        Self { client }
    }
}

#[async_trait]
impl HttpRequestor for WSHttpRequestor {
    async fn request_ext(
        &self,
        req: &mut Option<Request>,
        conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response> {
        let begin = std::time::Instant::now();

        // 选择一个ws session
        let mut session = self.client.select_session();
        if session.is_none() {
            error!("local ws disconnected! now will retry once");
            self.client.retry();
            async_std::task::sleep(std::time::Duration::from_secs(2)).await;

            session = self.client.select_session();
            if session.is_none() {
                error!("local ws disconnected! now will end with error");
                return Err(BuckyError::from(BuckyErrorCode::ConnectFailed));
            }
        }
        let session = session.unwrap();

        debug!(
            "will http-ws request via sid={}, url={}",
            session.sid(),
            req.as_ref().unwrap().url()
        );

        if let Some(conn_info) = conn_info {
            *conn_info = HttpRequestConnectionInfo::Tcp(session.conn_info().to_owned());
        }

        // request编码到buffer
        let req = req.take().unwrap();
        let mut encoder = async_h1::client::Encoder::new(req);
        let mut buf = vec![];
        encoder.read_to_end(&mut buf).await.map_err(|e| {
            let msg = format!(
                "encode http request to buffer error! sid={}, during={}ms, {}",
                session.sid(),
                begin.elapsed().as_millis(),
                e
            );
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        // 发起请求并等待应答
        let resp_buffer = session
            .requestor()
            .post_bytes_req(HTTP_CMD_REQUEST, buf)
            .await?;
        let resp_reader = async_std::io::Cursor::new(resp_buffer);
        let resp = async_h1::client::decode(resp_reader).await.map_err(|e| {
            let msg = format!(
                "decode http response from buffer error! sid={}, during={}ms, {}",
                session.sid(),
                begin.elapsed().as_millis(),
                e
            );
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        info!(
            "http-ws request to {} via sid={} success! during={}ms",
            self.remote_addr(),
            session.sid(),
            begin.elapsed().as_millis()
        );

        Ok(resp)
    }

    fn remote_addr(&self) -> String {
        self.client.service_addr().to_string()
    }

    fn remote_device(&self) -> Option<DeviceId> {
        None
    }

    fn clone_requestor(&self) -> Box<dyn HttpRequestor> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum RequestorRetryStrategy {
    EqualInterval,
    ExpInterval,
}

pub struct RequestorWithRetry {
    requestor: Box<dyn HttpRequestor>,
    retry_count: u32,
    retry_strategy: RequestorRetryStrategy,
    timeout: Option<Duration>,
}

impl Clone for RequestorWithRetry {
    fn clone(&self) -> Self {
        Self {
            requestor: self.requestor.clone_requestor(),
            retry_count: self.retry_count,
            retry_strategy: self.retry_strategy.clone(),
            timeout: None,
        }
    }
}

impl RequestorWithRetry {
    pub fn new(
        requestor: Box<dyn HttpRequestor>,
        retry_count: u32,
        retry_strategy: RequestorRetryStrategy,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            requestor,
            retry_count,
            retry_strategy,
            timeout,
        }
    }
}

#[async_trait]
impl HttpRequestor for RequestorWithRetry {
    async fn request_ext(
        &self,
        req: &mut Option<Request>,
        mut conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response> {
        let mut retry_count = 0;
        let info = &mut conn_info;
        loop {
            let ret = if let Some(t) = &self.timeout {
                self.requestor
                    .request_ext_timeout(req, t.clone(), info.as_deref_mut())
                    .await
            } else {
                self.requestor.request_ext(req, info.as_deref_mut()).await
            };

            match ret {
                Ok(resp) => break Ok(resp),
                Err(e) => {
                    // 只对连接失败的错误进行重试
                    // 请求超时的错误不能重试，req已经被消耗掉了
                    if e.code() != BuckyErrorCode::ConnectFailed {
                        break Err(e);
                    }

                    // 连接失败情况下，req不会被消耗，所以可以用做下次重试
                    assert!(req.is_some());

                    if retry_count >= self.retry_count {
                        warn!(
                            "bdt connect to {} extend max retry limit",
                            self.requestor.remote_addr()
                        );
                        break Err(e);
                    }

                    retry_count += 1;
                    let secd = match self.retry_strategy {
                        RequestorRetryStrategy::EqualInterval => 2_u64 * retry_count as u64,
                        RequestorRetryStrategy::ExpInterval => 2_u64.pow(retry_count),
                    };

                    warn!(
                        "bdt connect to {} error, now will retry after {} secs",
                        self.requestor.remote_addr(),
                        secd
                    );

                    async_std::task::sleep(Duration::from_secs(secd)).await;
                }
            }
        }
    }

    fn remote_addr(&self) -> String {
        self.requestor.remote_addr()
    }

    fn remote_device(&self) -> Option<DeviceId> {
        self.requestor.remote_device()
    }

    fn clone_requestor(&self) -> Box<dyn HttpRequestor> {
        self.requestor.clone_requestor()
    }
}
