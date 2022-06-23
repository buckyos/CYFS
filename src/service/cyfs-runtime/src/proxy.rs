use super::stack::CyfsStackInsConfig;
use crate::file_cache::FileCacheRecevier;
use crate::mime::*;
use cyfs_base::*;
use cyfs_stack_loader::{CyfsStack, HttpRequestSource, HttpServerHandlerRef};
use ood_control::OOD_CONTROLLER;

use async_trait::async_trait;
use http_types::headers::HeaderValue;
use http_types::StatusCode;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tide::listener::Listener;
use tide::security::{CorsMiddleware, Origin};

struct NonHttpServerInner {
    http_server: HttpServerHandlerRef,
    device_id: String,
}

#[derive(Clone)]
struct NonHttpServer(Arc<NonHttpServerInner>);

impl NonHttpServer {
    pub fn new(cyfs_stack: CyfsStack) -> Self {
        let http_server = cyfs_stack
            .interface()
            .as_ref()
            .unwrap()
            .get_http_tcp_server();
        let device_id = cyfs_stack.local_device_id().to_string();

        Self(Arc::new(NonHttpServerInner {
            http_server,
            device_id,
        }))
    }
}

struct NonForward {
    owner: CyfsProxy,
}

impl NonForward {
    pub fn new(owner: CyfsProxy) -> Self {
        Self { owner }
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for NonForward
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let addr = match req.peer_addr() {
            Some(addr) => addr.parse().unwrap(),
            None => "127.0.0.1:0".parse().unwrap(),
        };

        let url = req.url().clone();
        info!("recv non req: {}, {}", addr, url);

        let resp = match self.owner.non_handler() {
            Some(handler) => {
                let mut req: http_types::Request = req.into();

                // http请求都是同机请求，需要设定为当前device
                req.insert_header(
                    cyfs_base::CYFS_REMOTE_DEVICE,
                    handler.0.device_id.to_string(),
                );

                let source = HttpRequestSource::Local(addr);
                match handler.0.http_server.respond(source, req).await {
                    Ok(mut resp) => {
                        if resp.status().is_success() {
                            MimeHelper::try_set_mime(url, &mut resp).await;
                        }
                        resp.into()
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            None => {
                let mut resp = tide::Response::new(StatusCode::Forbidden);

                let msg = if ood_control::OOD_CONTROLLER.is_bind() {
                    format!("cyfs runtime stack's init is not complete yet!")
                } else {
                    format!("cyfs runtime device not bind yet!")
                };

                resp.set_body(msg);
                resp
            }
        };

        Ok(resp)
    }
}

#[derive(Serialize, Deserialize)]
struct StatusInfo {
    version: String,
    channel: String,
    target: String,
    is_bind: bool,
    is_mobile_stack: bool,
    anonymous: bool,
    random_id: bool,
}

struct StatusHelper {
    proxy: CyfsProxy,
}

impl StatusHelper {
    pub fn new(proxy: CyfsProxy) -> Self {
        Self { proxy }
    }

    fn gen_status(&self) -> StatusInfo {
        let ret = StatusInfo {
            version: cyfs_base::get_version().to_owned(),
            channel: cyfs_base::get_channel().to_string(),
            target: cyfs_base::get_target().to_string(),

            is_bind: OOD_CONTROLLER.is_bind(),
            is_mobile_stack: self.proxy.stack_config.is_mobile_stack,
            anonymous: self.proxy.stack_config.anonymous,
            random_id: self.proxy.stack_config.random_id,
        };

        ret
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for StatusHelper
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, _req: ::tide::Request<State>) -> tide::Result {
        let mut resp = tide::Response::new(StatusCode::Ok);
        resp.set_content_type("application/json");

        let status = self.gen_status();
        resp.set_body(serde_json::to_string_pretty(&status).unwrap());
        Ok(resp)
    }
}

pub(crate) struct CyfsProxyInner {
    static_root: PathBuf,
    non_http_server: OnceCell<NonHttpServer>,
}

impl CyfsProxyInner {
    pub fn new() -> Self {
        let static_root;
        #[cfg(target_os = "android")]
        {
            static_root = ::cyfs_util::get_cyfs_root_path().join("www");
            info!("set static web dir {}", static_root.display());
        }
        #[cfg(not(target_os = "android"))]
        {
            let root = std::env::current_exe().unwrap();
            let root = root.parent().unwrap().join("www");
            if root.is_dir() {
                static_root = root.canonicalize().unwrap();
            } else {
                static_root = root;
            }
        }

        Self {
            static_root,
            non_http_server: OnceCell::new(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct CyfsProxy {
    stack_config: CyfsStackInsConfig,
    inner: Arc<CyfsProxyInner>,
}

impl CyfsProxy {
    pub fn new(stack_config: &CyfsStackInsConfig) -> Self {
        assert!(stack_config.proxy_port > 0);
        Self {
            stack_config: stack_config.to_owned(),
            inner: Arc::new(CyfsProxyInner::new()),
        }
    }

    pub fn bind_non_stack(&self, cyfs_stack: CyfsStack) {
        let server = NonHttpServer::new(cyfs_stack);
        if let Err(_) = self.inner.non_http_server.set(server) {
            unreachable!();
        }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        let mut server = ::tide::new();
        let cors = CorsMiddleware::new()
            .allow_methods(
                "GET, POST, PUT, DELETE, OPTIONS"
                    .parse::<HeaderValue>()
                    .unwrap(),
            )
            .allow_origin(Origin::from("*"))
            .allow_credentials(true)
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .expose_headers("*".parse::<HeaderValue>().unwrap());
        server.with(cors);
        self.register(&mut server)?;

        let addr = format!("127.0.0.1:{}", self.stack_config.proxy_port);
        let mut listener = server.bind(&addr).await.map_err(|e| {
            error!("runtime proxy bind error! addr={}, {}", addr, e);
            e
        })?;

        for info in listener.info().iter() {
            info!(
                "runtime http server listening on addr={}, info={}",
                addr, info
            );
        }

        async_std::task::spawn(async move {
            if let Err(e) = listener.accept().await {
                error!("http server accept error! addr={}, {}", addr, e);
            }
        });

        Ok(())
    }

    fn register(&self, server: &mut ::tide::Server<()>) -> BuckyResult<()> {
        if !self.inner.static_root.is_dir() {
            error!(
                "static dir now exists! dir={}",
                self.inner.static_root.display()
            );
        }

        server
            .at("/static")
            .serve_dir(&self.inner.static_root)
            .map_err(|e| {
                let msg = format!("serve static dir failed! {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InternalError, msg)
            })?;
        info!("serve static dir: {}", self.inner.static_root.display());

        server.at("/status").get(StatusHelper::new(self.clone()));

        let file_cache = FileCacheRecevier::new();
        server.at("/file-cache").post(file_cache);

        server.at("/*").get(NonForward::new(self.clone()));

        Ok(())
    }

    fn non_handler(&self) -> Option<&NonHttpServer> {
        self.inner.non_http_server.get()
    }
}
