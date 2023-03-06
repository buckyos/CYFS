use super::requestor::*;
use crate::base::*;
use crate::ws::*;
use cyfs_base::*;

use async_std::io::ReadExt;
use http_types::Url;
use http_types::{Request, Response};
use std::sync::Arc;

#[derive(Clone)]
struct WSHttpRequestorHandler {}
impl WSHttpRequestorHandler {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
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

#[async_trait::async_trait]
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
            warn!("local ws not yet established or disconnected! now will retry once");
            self.client.retry();
            async_std::task::sleep(std::time::Duration::from_secs(2)).await;

            session = self.client.select_session();
            if session.is_none() {
                let msg = format!("local ws not yet established or disconnected! now will end with error");
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::ConnectFailed, msg));
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
        let req  = self.add_default_headers(req);
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

    async fn stop(&self) {
        self.client.stop().await
    }
}
