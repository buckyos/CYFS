use super::handler::*;
use crate::non::*;
use crate::zone::ZoneManagerRef;
use cyfs_lib::*;

use async_trait::async_trait;
use tide::Response;

enum NONRequestType {
    PutObject,
    Get,
    PostObject,
    DeleteObject,
}

pub(crate) struct NONRequestHandlerEndpoint {
    zone_manager: ZoneManagerRef,
    protocol: RequestProtocol,
    req_type: NONRequestType,
    handler: NONRequestHandler,
}

impl NONRequestHandlerEndpoint {
    fn new(
        zone_manager: ZoneManagerRef,
        protocol: RequestProtocol,
        req_type: NONRequestType,
        handler: NONRequestHandler,
    ) -> Self {
        Self {
            zone_manager,
            protocol,
            req_type,
            handler,
        }
    }

    async fn process_request<State: Send>(&self, req: ::tide::Request<State>) -> Response {
        let req = match NONInputHttpRequest::new(&self.zone_manager, &self.protocol, req).await {
            Ok(v) => v,
            Err(resp) => return resp,
        };

        match self.req_type {
            NONRequestType::Get => self.handler.process_get_request(req).await,
            NONRequestType::PutObject => self.handler.process_put_object_request(req).await,
            NONRequestType::PostObject => self.handler.process_post_object_request(req).await,
            NONRequestType::DeleteObject => self.handler.process_delete_object_request(req).await,
        }
    }

    pub fn register_server(
        zone_manager: &ZoneManagerRef,
        protocol: &RequestProtocol,
        handler: &NONRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
        // get_object/select_object
        server.at("/non/").get(NONRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            NONRequestType::Get,
            handler.clone(),
        ));
        server.at("/non").get(NONRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            NONRequestType::Get,
            handler.clone(),
        ));

        // put_object
        server.at("/non/").put(NONRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            NONRequestType::PutObject,
            handler.clone(),
        ));
        server.at("/non").put(NONRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            NONRequestType::PutObject,
            handler.clone(),
        ));

        // post_object
        server.at("/non/").post(NONRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            NONRequestType::PostObject,
            handler.clone(),
        ));
        server.at("/non").post(NONRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            NONRequestType::PostObject,
            handler.clone(),
        ));

        // delete_object
        server
            .at("/non/")
            .delete(NONRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                NONRequestType::DeleteObject,
                handler.clone(),
            ));
        server
            .at("/non")
            .delete(NONRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                NONRequestType::DeleteObject,
                handler.clone(),
            ));
    }
}

#[async_trait]
impl<State: Send> tide::Endpoint<State> for NONRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
