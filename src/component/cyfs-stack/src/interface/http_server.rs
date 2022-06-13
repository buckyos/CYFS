use super::auth::InterfaceAuth;
use super::translator::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct HttpDefaultHandler {
    block_list: Arc<std::collections::HashSet<String>>,
}

impl Default for HttpDefaultHandler {
    fn default() -> Self {
        let mut block_list = std::collections::HashSet::new();
        block_list.insert("/favicon.ico".to_owned());
        block_list.insert("/index.html".to_owned());
        block_list.insert("/".to_owned());

        Self {
            block_list: Arc::new(block_list),
        }
    }
}

impl HttpDefaultHandler {
    pub fn process(&self, req: &http_types::Request) -> Option<http_types::Response> {
        if self.block_list.contains(req.url().path()) {
            return Some(RequestorHelper::new_response(
                http_types::StatusCode::NotFound,
            ));
        }

        None
    }
}

#[derive(Clone, Debug)]
pub enum HttpRequestSource {
    Remote((DeviceId, u16)),
    Local(SocketAddr),
}

#[async_trait::async_trait]
pub trait HttpServerHandler: Send + Sync {
    async fn respond(
        &self,
        source: HttpRequestSource,
        req: http_types::Request,
    ) -> http_types::Result<http_types::Response>;
}

pub type HttpServerHandlerRef = Arc<Box<dyn HttpServerHandler>>;

#[derive(Clone)]
pub(crate) struct RawHttpServer {
    server: Arc<::tide::Server<()>>,
}

impl RawHttpServer {
    pub(crate) fn new(server: ::tide::Server<()>) -> Self {
        Self {
            server: Arc::new(server),
        }
    }

    pub fn into(self) -> HttpServerHandlerRef {
        Arc::new(Box::new(self))
    }
}

#[async_trait::async_trait]
impl HttpServerHandler for RawHttpServer {
    async fn respond(
        &self,
        _source: HttpRequestSource,
        req: http_types::Request,
    ) -> http_types::Result<http_types::Response> {
        self.server.respond(req).await
    }
}

#[derive(Clone)]
pub(crate) struct DefaultHttpServer {
    handler: HttpServerHandlerRef,
    url_translator: Option<UrlTransaltor>,
    default_handler: HttpDefaultHandler,
}

impl DefaultHttpServer {
    pub(crate) fn new(
        handler: HttpServerHandlerRef,
        url_translator: Option<UrlTransaltor>,
        default_handler: HttpDefaultHandler,
    ) -> Self {
        Self {
            handler,
            url_translator,
            default_handler,
        }
    }

    pub fn into(self) -> HttpServerHandlerRef {
        Arc::new(Box::new(self))
    }
}

#[async_trait::async_trait]
impl HttpServerHandler for DefaultHttpServer {
    async fn respond(
        &self,
        source: HttpRequestSource,
        mut req: http_types::Request,
    ) -> http_types::Result<http_types::Response> {
        // 过滤一些错误请求
        if let Some(resp) = self.default_handler.process(&req) {
            return Ok(resp);
        }

        if let Some(url_translator) = &self.url_translator {
            match url_translator.translate_url(&mut req).await {
                Ok(_) => self.handler.respond(source, req).await,
                Err(e) => Ok(RequestorHelper::trans_error(e)),
            }
        } else {
            self.handler.respond(source, req).await
        }
    }
}

// 带dec_id来源校验的http_server服务处理器
// 请求里面必须带和注册权限时候匹配的dec_id
pub(crate) struct AuthenticatedHttpServer {
    handler: HttpServerHandlerRef,
    auth: InterfaceAuth,
}

impl AuthenticatedHttpServer {
    pub fn new(handler: HttpServerHandlerRef, auth: InterfaceAuth) -> Self {
        Self {
            handler,
            auth,
        }
    }

    pub fn into(self) -> HttpServerHandlerRef {
        Arc::new(Box::new(self))
    }

    fn check_dec(
        &self,
        source: &HttpRequestSource,
        req: &mut http_types::Request,
    ) -> BuckyResult<()> {
        // extract dec_id from headers, must been existed!
        let dec_id: ObjectId = RequestorHelper::decode_header(req, cyfs_base::CYFS_DEC_ID)?;

        self.auth.check_dec(&dec_id, source)
    }
}

#[async_trait::async_trait]
impl HttpServerHandler for AuthenticatedHttpServer {
    async fn respond(
        &self,
        source: HttpRequestSource,
        mut req: http_types::Request,
    ) -> http_types::Result<http_types::Response> {
        if let Err(e) = self.check_dec(&source, &mut req) {
            return Ok(RequestorHelper::trans_error(e));
        }

        self.handler.respond(source, req).await
    }
}