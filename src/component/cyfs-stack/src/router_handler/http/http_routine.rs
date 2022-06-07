use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;

use async_std::net::TcpStream;
use async_trait::async_trait;
use http_types::{Method, Request, StatusCode, Url};
use std::fmt;
use std::marker::PhantomData;
use std::net::SocketAddr;

pub(crate) struct RouterHandlerHttpRoutine<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    _phantom_req: PhantomData<REQ>,
    _phantom_resp: PhantomData<RESP>,

    path: String,

    // 形如127.0.0.1:1080
    handler: String,
    local_addr: SocketAddr,
}

#[async_trait]
impl<REQ, RESP> EventListenerAsyncRoutine<RouterHandlerRequest<REQ, RESP>, RouterHandlerResponse<REQ, RESP>>
    for RouterHandlerHttpRoutine<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    async fn call(&self, param: &RouterHandlerRequest<REQ, RESP>) -> BuckyResult<RouterHandlerResponse<REQ, RESP>> {
        let body = param.encode_string();

        self.post_with_timeout(param, body).await
    }
}

impl<REQ, RESP> RouterHandlerHttpRoutine<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    pub fn new(chain: &RouterHandlerChain, categroy: &RouterHandlerCategory, id: &str, handler: &str) -> BuckyResult<Self> {
        debug!(
            "new http router handler routine: chain={}, category={}, id={}, handler={}",
            chain, categroy, id, handler
        );

        let url = Url::parse(handler).map_err(|e| {
            let msg = format!("invalid http router routine url: {} {}", handler, e);
            error!("{}", msg);

            BuckyError::from((BuckyErrorCode::InvalidFormat, msg))
        })?;

        // 格式为[base]/handler_chain/handler_category/handler_id
        let sub_path = format!("{}/{}/{}/", chain.to_string(), categroy.to_string(), id);
        let url = url.join(&sub_path).unwrap();

        let host = url.host_str().ok_or_else(|| {
            let msg = format!("invalid http router routine url, host not found: {}", url);
            error!("{}", msg);

            BuckyError::from((BuckyErrorCode::InvalidFormat, msg))
        })?;

        let port = url.port().ok_or_else(|| {
            let msg = format!("invalid http router routine url, port not found: {}", url);
            error!("{}", msg);

            BuckyError::from((BuckyErrorCode::InvalidFormat, msg))
        })?;

        let path = url.path().to_owned();

        let local_addr = format!("{}:{}", host, port).parse().unwrap();

        let ret = Self {
            _phantom_req: PhantomData,
            _phantom_resp: PhantomData,
            path,
            local_addr,
            handler: handler.to_owned(),
        };

        Ok(ret)
    }

    async fn post_with_timeout<T>(&self, param: &RouterHandlerRequest<REQ, RESP>, body: String) -> BuckyResult<T>
    where
        T: JsonCodec<T>,
        REQ: fmt::Display,
        RESP: fmt::Display,
    {
        match async_std::future::timeout(ROUTER_HANDLER_ROUTINE_TIMEOUT.clone(), async { self.post(param, body).await }).await {
            Ok(ret) => ret,
            Err(async_std::future::TimeoutError { .. }) => {
                let msg = format!("emit http routine timeout! {} {}", self.handler, self.path);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::Timeout, msg))
            }
        }
    }

    async fn post<T>(&self, param: &RouterHandlerRequest<REQ, RESP>, body: String) -> BuckyResult<T>
    where
        T: JsonCodec<T>,
        REQ: fmt::Display,
        RESP: fmt::Display,
    {
        let handler = self.handler.as_ref();
        let url = Url::parse(handler).unwrap();
        let url = url.join(&self.path).unwrap();

        debug!(
            "will emit router http routine: {} {}",
            url.to_string(),
            param
        );

        let stream = TcpStream::connect(&self.local_addr).await.map_err(|e| {
            let msg = format!(
                "tcp connect to http routine interface failed! {} {}",
                self.local_addr, e
            );
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        // 默认使用POST方法
        let mut req = Request::new(Method::Post, url);
        req.set_body(body);

        let mut resp = ::async_h1::connect(stream, req).await.map_err(|e| {
            error!(
                "http connect to http routine interface error! {} {}",
                handler, e
            );
            BuckyError::from(e)
        })?;

        match resp.status() {
            StatusCode::Ok => {
                let resp_str = resp.body_string().await.map_err(|e| {
                    let msg = format!("parse http routine resp body error! err={}", e);
                    error!("{}", msg);

                    BuckyError::from(msg)
                })?;

                T::decode_string(&resp_str)
            }

            StatusCode::NotFound => {
                warn!("http routine request but not found!");
                Err(BuckyError::from(BuckyErrorCode::NotFound))
            }
            StatusCode::BadRequest => {
                error!("http routine bad request!");
                Err(BuckyError::from(BuckyErrorCode::InvalidFormat))
            }
            v @ _ => {
                let msg = format!("http routine error! status={}", v);
                error!("{}", msg);
                Err(BuckyError::from(msg))
            }
        }
    }
}
