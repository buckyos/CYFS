use super::handler::*;
use cyfs_lib::RequestorHelper;

use async_trait::async_trait;
use tide::{Response, StatusCode};

enum ZoneSyncRequestType {
    Ping,
    Diff,
    Objects,
    Chunks,
}

pub(crate) struct ZoneSyncRequestHandlerEndpoint {
    req_type: ZoneSyncRequestType,
    handler: ZoneSyncRequestHandler,
}

impl ZoneSyncRequestHandlerEndpoint {
    fn new(req_type: ZoneSyncRequestType, handler: ZoneSyncRequestHandler) -> Self {
        Self { req_type, handler }
    }

    async fn process_request<State>(&self, mut req: ::tide::Request<State>) -> Response {
        match req.body_string().await {
            Ok(body) => match self.req_type {
                ZoneSyncRequestType::Ping => self.handler.process_ping_request(req, body).await,
                ZoneSyncRequestType::Diff => self.handler.process_diff_request(req, body).await,
                ZoneSyncRequestType::Objects => {
                    self.handler.process_objects_request(req, body).await
                }
                ZoneSyncRequestType::Chunks => {
                    self.handler.process_chunks_request(req, body).await
                }
            },

            Err(e) => {
                error!("read sync request body error! err={}", e);

                RequestorHelper::new_response(StatusCode::BadRequest).into()
            }
        }
    }

    pub fn register_server(handler: &ZoneSyncRequestHandler, server: &mut ::tide::Server<()>) {
        // ping
        server
            .at("/sync/ping/:device_id")
            .post(ZoneSyncRequestHandlerEndpoint::new(
                ZoneSyncRequestType::Ping,
                handler.clone(),
            ));

        // diff
        server
            .at("/sync/diff")
            .post(ZoneSyncRequestHandlerEndpoint::new(
                ZoneSyncRequestType::Diff,
                handler.clone(),
            ));

        // objects
        server
            .at("/sync/objects/")
            .get(ZoneSyncRequestHandlerEndpoint::new(
                ZoneSyncRequestType::Objects,
                handler.clone(),
            ));
        server
            .at("/sync/objects")
            .get(ZoneSyncRequestHandlerEndpoint::new(
                ZoneSyncRequestType::Objects,
                handler.clone(),
            ));

        // objects
        server
            .at("/sync/chunks/")
            .get(ZoneSyncRequestHandlerEndpoint::new(
                ZoneSyncRequestType::Chunks,
                handler.clone(),
            ));
        server
            .at("/sync/chunks")
            .get(ZoneSyncRequestHandlerEndpoint::new(
                ZoneSyncRequestType::Chunks,
                handler.clone(),
            ));
    }

    // 对外提供服务,同zone可访问
    pub fn register_zone_service(_handler: &ZoneSyncRequestHandler, _server: &mut ::tide::Server<()>) {

    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for ZoneSyncRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
