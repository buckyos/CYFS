use super::requestor::*;
use cyfs_base::*;

use async_std::net::{SocketAddr, TcpStream};
use http_types::{Request, Response};
use std::str::FromStr;

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

#[async_trait::async_trait]
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

    async fn stop(&self) {
        // do nothing
    }
}
