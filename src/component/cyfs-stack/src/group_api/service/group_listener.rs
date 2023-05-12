use cyfs_lib::RequestProtocol;
use tide::Response;

use crate::{non::NONInputHttpRequest, ZoneManagerRef};

use super::GroupRequestHandler;

enum GroupRequestType {
    StartService,
    PushProposal,
}

pub(crate) struct GroupRequestHandlerEndpoint {
    zone_manager: ZoneManagerRef,
    protocol: RequestProtocol,
    req_type: GroupRequestType,
    handler: GroupRequestHandler,
}

impl GroupRequestHandlerEndpoint {
    fn new(
        zone_manager: ZoneManagerRef,
        protocol: RequestProtocol,
        req_type: GroupRequestType,
        handler: GroupRequestHandler,
    ) -> Self {
        Self {
            zone_manager,
            protocol,
            req_type,
            handler,
        }
    }

    async fn process_request<State: Send>(&self, req: tide::Request<State>) -> Response {
        let req = match NONInputHttpRequest::new(&self.zone_manager, &self.protocol, req).await {
            Ok(v) => v,
            Err(resp) => return resp,
        };
        match self.req_type {
            GroupRequestType::StartService => self.handler.process_start_service(req).await,
            GroupRequestType::PushProposal => self.handler.process_push_proposal(req).await,
        }
    }

    pub fn register_server(
        zone_manager: &ZoneManagerRef,
        protocol: &RequestProtocol,
        handler: &GroupRequestHandler,
        server: &mut tide::Server<()>,
    ) {
        server.at("/group/start-service").put(Self::new(
            zone_manager.clone(),
            protocol.to_owned(),
            GroupRequestType::StartService,
            handler.clone(),
        ));

        server.at("group/push-proposal").put(Self::new(
            zone_manager.clone(),
            protocol.to_owned(),
            GroupRequestType::PushProposal,
            handler.clone(),
        ));
    }
}

#[async_trait::async_trait]
impl<State> tide::Endpoint<State> for GroupRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
