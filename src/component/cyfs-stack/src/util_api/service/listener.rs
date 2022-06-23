use super::handler::*;
use crate::non::NONInputHttpRequest;
use cyfs_lib::*;

use async_trait::async_trait;
use tide::Response;

enum UtilRequestType {
    ResolveOOD,
    GetDevice,
    GetZone,
    GetOODstatus,
    GetDeviceStaticInfo,
    GetSystemInfo,
    GetNOCInfo,
    GetNetworkAccessInfo,
    GetVersionInfo,
    BuildFile,
    BuildDirFromObjectMap,
}

pub(crate) struct UtilRequestHandlerEndpoint {
    protocol: NONProtocol,
    req_type: UtilRequestType,
    handler: UtilRequestHandler,
}

impl UtilRequestHandlerEndpoint {
    fn new(protocol: NONProtocol, req_type: UtilRequestType, handler: UtilRequestHandler) -> Self {
        Self {
            protocol,
            req_type,
            handler,
        }
    }

    async fn process_request<State>(&self, req: ::tide::Request<State>) -> Response {
        let req = NONInputHttpRequest::new(&self.protocol, req);

        match self.req_type {
            UtilRequestType::ResolveOOD => self.handler.process_resolve_ood_request(req).await,
            UtilRequestType::GetDevice => self.handler.process_get_device(req).await,
            UtilRequestType::GetZone => self.handler.process_get_zone(req).await,
            UtilRequestType::GetOODstatus => self.handler.process_get_ood_status_request(req).await,
            UtilRequestType::GetDeviceStaticInfo => {
                self.handler
                    .process_get_device_static_info_request(req)
                    .await
            }
            UtilRequestType::GetSystemInfo => {
                self.handler.process_get_system_info_request(req).await
            }
            UtilRequestType::GetNOCInfo => self.handler.process_get_noc_info_request(req).await,
            UtilRequestType::GetNetworkAccessInfo => {
                self.handler
                    .process_get_network_access_info_request(req)
                    .await
            }
            UtilRequestType::GetVersionInfo => {
                self.handler.process_get_version_info_request(req).await
            }
            UtilRequestType::BuildFile => {
                self.handler.process_build_file_request(req).await
            }
            UtilRequestType::BuildDirFromObjectMap => {
                self.handler.process_build_dir_from_object_map_request(req).await
            }
        }
    }

    pub fn register_server(
        protocol: &NONProtocol,
        handler: &UtilRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
        // get_device
        server.at("/util/device").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetDevice,
            handler.clone(),
        ));

        server.at("/util/device/").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetDevice,
            handler.clone(),
        ));

        server.at("/util/device/*must").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetDevice,
            handler.clone(),
        ));

        // get_zone
        server.at("/util/zone").post(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetZone,
            handler.clone(),
        ));

        server.at("/util/zone/").post(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetZone,
            handler.clone(),
        ));

        server.at("/util/zone/*must").post(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetZone,
            handler.clone(),
        ));

        // resolve_ood
        server
            .at("/util/resolve_ood/*must")
            .get(Self::new(protocol.to_owned(),UtilRequestType::ResolveOOD, handler.clone()));

        // get_device_static_info
        server.at("/util/device_static_info").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetDeviceStaticInfo,
            handler.clone(),
        ));
        server.at("/util/device_static_info/").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetDeviceStaticInfo,
            handler.clone(),
        ));
        server.at("/util/device_static_info/*must").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetDeviceStaticInfo,
            handler.clone(),
        ));

        // get_system_info
        server.at("/util/system_info").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetSystemInfo,
            handler.clone(),
        ));
        server.at("/util/system_info/").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetSystemInfo,
            handler.clone(),
        ));
        server.at("/util/system_info/*must").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetSystemInfo,
            handler.clone(),
        ));

        // get_ood_status
        server
            .at("/util/ood_status")
            .get(Self::new(protocol.to_owned(),UtilRequestType::GetOODstatus, handler.clone()));
        server
            .at("/util/ood_status/")
            .get(Self::new(protocol.to_owned(),UtilRequestType::GetOODstatus, handler.clone()));
        server
            .at("/util/ood_status/*must")
            .get(Self::new(protocol.to_owned(),UtilRequestType::GetOODstatus, handler.clone()));

        // noc_info
        server.at("/util/noc_info").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetNOCInfo,
            handler.clone(),
        ));
        server.at("/util/noc_info/").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetNOCInfo,
            handler.clone(),
        ));
        server.at("/util/noc_info/*must").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetNOCInfo,
            handler.clone(),
        ));

        // network_access_info
        server.at("/util/network_access_info").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetNetworkAccessInfo,
            handler.clone(),
        ));
        server.at("/util/network_access_info/").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetNetworkAccessInfo,
            handler.clone(),
        ));
        server.at("/util/network_access_info/*must").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetNetworkAccessInfo,
            handler.clone(),
        ));

        // get_version
        server.at("/util/version_info").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetVersionInfo,
            handler.clone(),
        ));
        server.at("/util/version_info/").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetVersionInfo,
            handler.clone(),
        ));
        server.at("/util/version_info/*must").get(Self::new(
            protocol.to_owned(),
            UtilRequestType::GetVersionInfo,
            handler.clone(),
        ));

        server.at("/util/build_file").post(Self::new(
            protocol.to_owned(),
            UtilRequestType::BuildFile,
            handler.clone(),
        ));
        server.at("/util/build_file/*must").post(Self::new(
            protocol.to_owned(),
            UtilRequestType::BuildFile,
            handler.clone(),
        ));

        server.at("/util/build_dir_from_object_map").post(Self::new(
            protocol.to_owned(),
            UtilRequestType::BuildDirFromObjectMap,
            handler.clone(),
        ));
        server.at("/util/build_dir_from_object_map/*must").post(Self::new(
            protocol.to_owned(),
            UtilRequestType::BuildDirFromObjectMap,
            handler.clone(),
        ));
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for UtilRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
