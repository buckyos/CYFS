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

    AddObjectMeta,
    RemoveObjectMeta,
    ClearObjectMeta,

    AddPathConfig,
    RemovePathConfig,
    ClearPathConfig,
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

            GlobalStateMetaRequestType::AddObjectMeta => {
                self.handler.process_add_object_meta_request(req).await
            }
            GlobalStateMetaRequestType::RemoveObjectMeta => {
                self.handler.process_remove_object_meta_request(req).await
            }
            GlobalStateMetaRequestType::ClearObjectMeta => {
                self.handler.process_clear_object_meta_request(req).await
            }

            GlobalStateMetaRequestType::AddPathConfig => {
                self.handler.process_add_path_config_request(req).await
            }
            GlobalStateMetaRequestType::RemovePathConfig => {
                self.handler.process_remove_path_config_request(req).await
            }
            GlobalStateMetaRequestType::ClearPathConfig => {
                self.handler.process_clear_path_config_request(req).await
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

        let path = format!("/{}/meta/object-meta", root_seg);
        // add_object_meta
        server
            .at(&path)
            .put(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::AddObjectMeta,
                handler.clone(),
            ));

        // remove_object_meta
        server
            .at(&path)
            .delete(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::RemoveObjectMeta,
                handler.clone(),
            ));

        // clear_object_meta
        let path = format!("/{}/meta/object-metas", root_seg);
        server
            .at(&path)
            .delete(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::ClearObjectMeta,
                handler.clone(),
            ));

        let path = format!("/{}/meta/path-config", root_seg);
        // add_path_config
        server
            .at(&path)
            .put(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::AddPathConfig,
                handler.clone(),
            ));

        // remove_path_config
        server
            .at(&path)
            .delete(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::RemovePathConfig,
                handler.clone(),
            ));

        // clear_path_config
        let path = format!("/{}/meta/path-configs", root_seg);
        server
            .at(&path)
            .delete(GlobalStateMetaRequestHandlerEndpoint::new(
                zone_manager.clone(),
                protocol.to_owned(),
                GlobalStateMetaRequestType::ClearPathConfig,
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
