use super::handler::*;
use crate::non::*;
use crate::zone::ZoneManagerRef;
use cyfs_lib::*;

use async_trait::async_trait;
use tide::Response;

enum GlobalStateMetaRequestType {
    AddAccess,
    RemoveAccess,
    ClearAccess,

    AddLink,
    RemoveLink,
    ClearLink,
}

pub(crate) struct GlobalStateMetaRequestHandlerEndpoint {
    zone_manager: ZoneManagerRef,
    protocol: RequestProtocol,
    req_type: GlobalStateMetaRequestType,
    handler: GlobalStateMetaRequestHandler,
}

impl GlobalStateMetaRequestHandlerEndpoint {
    fn new(
        zone_manager: ZoneManagerRef,
        protocol: RequestProtocol,
        req_type: GlobalStateMetaRequestType,
        handler: GlobalStateMetaRequestHandler,
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
            GlobalStateMetaRequestType::AddAccess => {
                self.handler.process_add_access_request(req).await
            }
            GlobalStateMetaRequestType::RemoveAccess => {
                self.handler.process_remove_access_request(req).await
            }
            GlobalStateMetaRequestType::ClearAccess => {
                self.handler.process_clear_access_request(req).await
            }

            GlobalStateMetaRequestType::AddLink => self.handler.process_add_link_request(req).await,
            GlobalStateMetaRequestType::RemoveLink => {
                self.handler.process_remove_link_request(req).await
            }
            GlobalStateMetaRequestType::ClearLink => {
                self.handler.process_clear_link_request(req).await
            }
        }
    }

    pub fn register_server(
        zone_manager: &ZoneManagerRef,
        protocol: &RequestProtocol,
        root_seg: &str,
        handler: &GlobalStateMetaRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
        let path = format!("/{}/meta/access", root_seg);

        // add_access
        server
            .at(&path)
            .put(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::AddAccess,
                handler.clone(),
            ));

        // remove_access
        server
            .at(&path)
            .delete(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::RemoveAccess,
                handler.clone(),
            ));

        // clear_access
        let path = format!("/{}/meta/accesses", root_seg);
        server
            .at(&path)
            .delete(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::ClearAccess,
                handler.clone(),
            ));

        let path = format!("/{}/meta/link", root_seg);

        // add_link
        server
            .at(&path)
            .put(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::AddLink,
                handler.clone(),
            ));

        // remove_link
        server
            .at(&path)
            .delete(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::RemoveLink,
                handler.clone(),
            ));

        // clear_link
        let path = format!("/{}/meta/links", root_seg);
        server
            .at(&path)
            .delete(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::ClearLink,
                handler.clone(),
            ));
    }
}

#[async_trait]
impl<State: Send> tide::Endpoint<State> for GlobalStateMetaRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
