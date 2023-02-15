use super::controller::*;
use super::request::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::SYSTEM_INFO_MANAGER;

use async_trait::async_trait;
use http_types::Url;
use std::sync::Arc;
use tide::{Response, StatusCode};

enum RequestType {
    Check,
    Bind,
    SystemInfo,
}

pub(crate) struct HandlerEndpoint {
    req_type: RequestType,
    handler: Controller,
    access_token: Option<String>,
}

impl HandlerEndpoint {
    fn new(req_type: RequestType, access_token: Option<String>, handler: Controller) -> Self {
        Self {
            req_type,
            access_token,
            handler,
        }
    }

    fn check_token(&self, url: &Url) -> BuckyResult<()> {
        let pairs = url.query_pairs();
        let mut token = None;
        for (k, v) in pairs {
            if k == "access_token" {
                token = Some(v.into_owned());
                break;
            }
        }

        if token.is_none() {
            let msg = format!("access token token query not found! url={}", url);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        if self.access_token != token {
            let msg = format!("invalid access token: {}", token.unwrap());
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        Ok(())
    }

    async fn process_request<State>(&self, mut req: ::tide::Request<State>) -> Response {
        if self.access_token.is_some() {
            if let Err(e) = self.check_token(&req.url()) {
                return cyfs_lib::RequestorHelper::trans_error(e);
            }
        }

        match self.req_type {
            RequestType::Check => {
                let source: String =
                    RequestorHelper::decode_header(&req, ::cyfs_base::CYFS_REMOTE_DEVICE).unwrap();

                self.on_check_request(&source).await
            }
            RequestType::Bind => match req.body_json().await {
                Ok(info) => self.on_bind_request(info).await,
                Err(e) => {
                    let msg = format!("parse bind info error: {}", e);
                    error!("{}", msg);

                    Response::builder(StatusCode::BadRequest).body(msg).build()
                }
            },
            RequestType::SystemInfo => self.on_get_system_info_request().await,
        }
    }

    async fn on_check_request(&self, source: &str) -> Response {
        self.handler.on_check_request(source);

        let ret = self.handler.check().await;
        let content = serde_json::to_string(&ret).unwrap();
        Response::builder(200).body(content).build()
    }

    async fn on_bind_request(&self, info: ActivateInfo) -> Response {
        let result = self.handler.bind(info).await;

        let content = serde_json::to_string(&result).unwrap();
        Response::builder(200).body(content).build()
    }

    async fn on_get_system_info_request(&self) -> Response {
        let info = SYSTEM_INFO_MANAGER.get_system_info().await;

        let content = serde_json::to_string(&info).unwrap();
        Response::builder(200).body(content).build()
    }

    pub fn register_server(
        handler: &Controller,
        access_token: Option<String>,
        server: &mut ::tide::Server<()>,
    ) {
        // check
        server.at("/check").get(HandlerEndpoint::new(
            RequestType::Check,
            access_token.clone(),
            handler.to_owned(),
        ));

        server.at("/check/").get(HandlerEndpoint::new(
            RequestType::Check,
            access_token.clone(),
            handler.to_owned(),
        ));

        // bind
        server.at("/bind").post(HandlerEndpoint::new(
            RequestType::Bind,
            access_token.clone(),
            handler.to_owned(),
        ));

        server.at("/bind/").post(HandlerEndpoint::new(
            RequestType::Bind,
            access_token.clone(),
            handler.to_owned(),
        ));

        server.at("/activate").post(HandlerEndpoint::new(
            RequestType::Bind,
            access_token.clone(),
            handler.to_owned(),
        ));

        server.at("/activate/").post(HandlerEndpoint::new(
            RequestType::Bind,
            access_token.clone(),
            handler.to_owned(),
        ));

        // system_info
        server.at("/system_info").get(HandlerEndpoint::new(
            RequestType::SystemInfo,
            access_token.clone(),
            handler.to_owned(),
        ));

        server.at("/system_info/").get(HandlerEndpoint::new(
            RequestType::SystemInfo,
            access_token.clone(),
            handler.to_owned(),
        ));

        // external
        let all = handler.fetch_all_external_servers();
        for item in all {
            item.register(server);
        }
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for HandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> ::tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}

#[derive(Clone)]
pub struct HttpServer {
    server: Arc<::tide::Server<()>>,
}

impl HttpServer {
    pub fn new_server() -> ::tide::Server<()> {
        use http_types::headers::HeaderValue;
        use tide::security::{CorsMiddleware, Origin};

        let mut server = ::tide::new();

        let cors = CorsMiddleware::new()
            .allow_methods("GET, POST, OPTIONS".parse::<HeaderValue>().unwrap())
            .allow_origin(Origin::from("*"))
            .allow_credentials(true)
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .expose_headers("*".parse::<HeaderValue>().unwrap());
        server.with(cors);

        server
    }

    pub fn new(handler: &Controller, access_token: Option<String>) -> Self {
        let mut server = Self::new_server();
        HandlerEndpoint::register_server(handler, access_token, &mut server);

        Self {
            server: Arc::new(server),
        }
    }

    pub fn server(&self) -> &Arc<::tide::Server<()>> {
        &self.server
    }
    
    pub async fn respond(
        &self,
        req: http_types::Request,
    ) -> http_types::Result<http_types::Response> {
        self.server.respond(req).await
    }
}
