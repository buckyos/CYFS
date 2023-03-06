use crate::base::CYFS_CURRENT_API_EDITION;
use cyfs_base::*;

use async_std::net::SocketAddr;
use async_trait::async_trait;
use http_types::{Request, Response};
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

    fn add_default_headers(&self, mut req: Request) -> Request {
        req.insert_header(CYFS_API_EDITION, CYFS_CURRENT_API_EDITION.to_string());
        req
    }

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

    async fn stop(&self);
}

pub type HttpRequestorRef = Arc<Box<dyn HttpRequestor>>;

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

    async fn stop(&self) {
        self.requestor.stop().await
    }
}
