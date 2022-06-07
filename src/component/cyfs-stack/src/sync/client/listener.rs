use super::handler::*;
use cyfs_lib::RequestorHelper;

use async_trait::async_trait;
use tide::{Response, StatusCode};

enum DeviceSyncRequestType {
    Zone,
    Status,
}

pub(crate) struct DeviceSyncRequestHandlerEndpoint {
    req_type: DeviceSyncRequestType,
    handler: DeviceSyncRequestHandler,
}

impl DeviceSyncRequestHandlerEndpoint {
    fn new(req_type: DeviceSyncRequestType, handler: DeviceSyncRequestHandler) -> Self {
        Self { req_type, handler }
    }

    async fn process_request<State>(&self, mut req: ::tide::Request<State>) -> Response {
        match self.req_type {
            DeviceSyncRequestType::Zone => match req.body_string().await {
                Ok(body) => self.handler.process_zone_request(req, body).await,

                Err(e) => {
                    error!("read sync request body error! err={}", e);

                    RequestorHelper::new_response(StatusCode::BadRequest).into()
                }
            },
            DeviceSyncRequestType::Status => self.handler.process_state_request(req).await,
        }
    }

    pub fn register_server(handler: &DeviceSyncRequestHandler, server: &mut ::tide::Server<()>) {
        // ping
        server
            .at("/sync/zone/")
            .post(DeviceSyncRequestHandlerEndpoint::new(
                DeviceSyncRequestType::Zone,
                handler.clone(),
            ));

        server
            .at("/sync/zone")
            .post(DeviceSyncRequestHandlerEndpoint::new(
                DeviceSyncRequestType::Zone,
                handler.clone(),
            ));
    }

    // 对外提供的服务，同zone可访问
    pub fn register_zone_service(
        handler: &DeviceSyncRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
        // ping
        server
            .at("/sync/status/")
            .get(DeviceSyncRequestHandlerEndpoint::new(
                DeviceSyncRequestType::Status,
                handler.clone(),
            ));

        server
            .at("/sync/status")
            .get(DeviceSyncRequestHandlerEndpoint::new(
                DeviceSyncRequestType::Status,
                handler.clone(),
            ));

        server
            .at("/sync/status/")
            .post(DeviceSyncRequestHandlerEndpoint::new(
                DeviceSyncRequestType::Status,
                handler.clone(),
            ));

        server
            .at("/sync/status")
            .post(DeviceSyncRequestHandlerEndpoint::new(
                DeviceSyncRequestType::Status,
                handler.clone(),
            ));
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for DeviceSyncRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
