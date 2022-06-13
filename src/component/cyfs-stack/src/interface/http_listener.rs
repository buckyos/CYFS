use crate::acl::AclManagerRef;
use crate::crypto_api::*;
use crate::ndn_api::*;
use crate::non_api::*;
use crate::root_state_api::*;
use crate::router_handler::{
    RouterHandlerHttpHandler, RouterHandlerRequestHandlerEndpoint, RouterHandlersManager,
};
use crate::stack::ObjectServices;
use crate::sync::*;
use crate::trans_api::{TransRequestHandler, TransRequestHandlerEndpoint};
use crate::util_api::*;
use cyfs_base::*;
use cyfs_lib::{GlobalStateCategory, NONProtocol, RequestorHelper};

use std::sync::Arc;

fn new_server() -> ::tide::Server<()> {
    use http_types::headers::HeaderValue;
    use tide::security::{CorsMiddleware, Origin};

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

    server
}

async fn not_found_handler(req: tide::Request<()>) -> tide::Result {
    let msg = format!(
        "request not handled: method={}, path={}",
        req.method(),
        req.url().path()
    );
    warn!("{}", msg);
    let e = BuckyError::new(BuckyErrorCode::NotFound, msg);
    let resp = RequestorHelper::trans_error(e);
    Ok(resp)
}

fn default_handler(server: &mut ::tide::Server<()>) {
    server.at("/favicon.ico").all(not_found_handler);
    server.at("/").all(not_found_handler);
    server.at("*").all(not_found_handler);
}

pub(super) struct ObjectHttpListener {
    server: ::tide::Server<()>,
}

impl ObjectHttpListener {
    pub fn new(
        protocol: NONProtocol,
        services: &ObjectServices,
        router_handlers: &RouterHandlersManager,
        _acl: AclManagerRef,
        sync_server: Option<&Arc<ZoneSyncServer>>,
        sync_client: Option<&Arc<DeviceSyncClient>>,
        root_state: &GlobalStateService,
        local_cache: &GlobalStateLocalService,
    ) -> Self {
        let mut server = new_server();

        default_handler(&mut server);

        // router事件只支持本地的http协议
        if protocol == NONProtocol::HttpLocal {
            
            // router handlers
            let handler = RouterHandlerHttpHandler::new(protocol.clone(), router_handlers.clone());
            RouterHandlerRequestHandlerEndpoint::register_server(&handler, &mut server);
        }

        // sync提供的对外服务
        if let Some(sync_server) = sync_server {
            let handler = ZoneSyncRequestHandler::new(protocol.clone(), sync_server.clone());
            ZoneSyncRequestHandlerEndpoint::register_zone_service(&handler, &mut server);
        }
        if let Some(sync_client) = sync_client {
            let handler = DeviceSyncRequestHandler::new(protocol.clone(), sync_client.clone());
            DeviceSyncRequestHandlerEndpoint::register_zone_service(&handler, &mut server);
        }

        // root_state
        let handler = GlobalStateRequestHandler::new(root_state.clone_global_state_processor());
        GlobalStateRequestHandlerEndpoint::register_server(
            &protocol,
            GlobalStateCategory::RootState.as_str(),
            &handler,
            &mut server,
        );

        let handler = OpEnvRequestHandler::new(root_state.clone_op_env_processor());
        OpEnvRequestHandlerEndpoint::register_server(
            &protocol,
            GlobalStateCategory::RootState.as_str(),
            &handler,
            &mut server,
        );

        let handler = GlobalStateAccessRequestHandler::new(root_state.clone_access_processor());
        GlobalStateAccessRequestHandlerEndpoint::register_server(
            &protocol,
            GlobalStateCategory::RootState.as_str(),
            &handler,
            &mut server,
        );

        // local_cache
        let handler = GlobalStateRequestHandler::new(local_cache.clone_global_state_processor());
        GlobalStateRequestHandlerEndpoint::register_server(
            &protocol,
            GlobalStateCategory::LocalCache.as_str(),
            &handler,
            &mut server,
        );

        let handler = OpEnvRequestHandler::new(local_cache.clone_op_env_processor());
        OpEnvRequestHandlerEndpoint::register_server(
            &protocol,
            GlobalStateCategory::LocalCache.as_str(),
            &handler,
            &mut server,
        );

        let handler = GlobalStateAccessRequestHandler::new(root_state.clone_access_processor());
        GlobalStateAccessRequestHandlerEndpoint::register_server(
            &protocol,
            GlobalStateCategory::LocalCache.as_str(),
            &handler,
            &mut server,
        );

        // crypto service
        let handler = CryptoRequestHandler::new(services.crypto_service.clone_processor());
        CryptoRequestHandlerEndpoint::register_server(&protocol, &handler, &mut server);

        // non
        let handler = NONRequestHandler::new(services.non_service.clone_processor());
        NONRequestHandlerEndpoint::register_server(&protocol, &handler, &mut server);

        // ndn
        let handler = NDNRequestHandler::new(services.ndn_service.clone_processor());
        NDNRequestHandlerEndpoint::register_server(&protocol, &handler, &mut server);

        // util
        let handler = UtilRequestHandler::new(services.util_service.clone_processor());
        UtilRequestHandlerEndpoint::register_server(&protocol, &handler, &mut server);

        // trans service
        let handler =
            TransRequestHandler::new(protocol.clone(), services.trans_service.clone_processor());
        TransRequestHandlerEndpoint::register_server(&handler, &mut server);

        Self { server }
    }

    pub fn into_server(self) -> ::tide::Server<()> {
        self.server
    }
}

pub(super) struct SyncHttpListener {
    server: ::tide::Server<()>,
}

impl SyncHttpListener {
    pub fn new(
        protocol: NONProtocol,
        sync_server: Option<&Arc<ZoneSyncServer>>,
        sync_client: Option<&Arc<DeviceSyncClient>>,
    ) -> Self {
        let mut server = new_server();

        // sync只支持bdt协议
        match protocol {
            NONProtocol::HttpBdt => {
                if let Some(sync_server) = sync_server {
                    let handler =
                        ZoneSyncRequestHandler::new(protocol.clone(), sync_server.clone());
                    ZoneSyncRequestHandlerEndpoint::register_server(&handler, &mut server);
                }
                if let Some(sync_client) = sync_client {
                    let handler =
                        DeviceSyncRequestHandler::new(protocol.clone(), sync_client.clone());
                    DeviceSyncRequestHandlerEndpoint::register_server(&handler, &mut server);
                }
            }
            _ => {
                unreachable!();
            }
        }

        Self { server }
    }

    pub fn into_server(self) -> ::tide::Server<()> {
        self.server
    }
}
