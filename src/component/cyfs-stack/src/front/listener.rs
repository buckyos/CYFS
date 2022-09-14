use super::http_request::FrontInputHttpRequest;
use super::protocol::FrontProtocolHandlerRef;
use crate::zone::ZoneManagerRef;
use cyfs_lib::*;

use async_trait::async_trait;

#[derive(Clone, Copy)]
pub enum FrontRequestType {
    O,
    R,
    L,
    A,

    // treat as o protocol
    Any,
}

pub(crate) struct FrontRequestHandlerEndpoint {
    zone_manager: ZoneManagerRef,
    protocol: NONProtocol,
    req_type: FrontRequestType,
    handler: FrontProtocolHandlerRef,
}

impl FrontRequestHandlerEndpoint {
    fn new(
        zone_manager: ZoneManagerRef,
        protocol: NONProtocol,
        req_type: FrontRequestType,
        handler: FrontProtocolHandlerRef,
    ) -> Self {
        Self {
            zone_manager,
            protocol,
            req_type,
            handler,
        }
    }

    async fn process_request<State: Send>(&self, req: tide::Request<State>) -> tide::Response {
        let req = match FrontInputHttpRequest::new(&self.zone_manager, &self.protocol, req).await {
            Ok(v) => v,
            Err(resp) => return resp,
        };
        
        self.handler.process_request(self.req_type, req).await
    }

    pub fn register_server(
        zone_manager: &ZoneManagerRef,
        protocol: &NONProtocol,
        handler: &FrontProtocolHandlerRef,
        server: &mut ::tide::Server<()>,
    ) {
        // o protocol
        server.at("/o/*must").get(FrontRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            FrontRequestType::O,
            handler.clone(),
        ));

        // r protocol
        server.at("/r/*must").get(FrontRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            FrontRequestType::R,
            handler.clone(),
        ));
        server.at("/l/*must").get(FrontRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            FrontRequestType::L,
            handler.clone(),
        ));

        // a
        server.at("/a/*must").get(FrontRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            FrontRequestType::A,
            handler.clone(),
        ));

        // any
        server
            .at("/:name/*must")
            .get(FrontRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                FrontRequestType::Any,
                handler.clone(),
            ));

        server.at("/:name").get(FrontRequestHandlerEndpoint::new(
            zone_manager.clone(),
            protocol.to_owned(),
            FrontRequestType::Any,
            handler.clone(),
        ));
    }
}

#[async_trait]
impl<State: Send> tide::Endpoint<State> for FrontRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
