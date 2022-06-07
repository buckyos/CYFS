use super::http_server::*;
use super::translator::UrlTransaltor;
use super::{
    ObjectHttpBdtListener, ObjectHttpListener, ObjectHttpTcpListener, ObjectListener,
    WebSocketEventInterface,
};
use crate::acl::AclManagerRef;
use crate::app::AppService;
use crate::app::AuthenticatedAppList;
use crate::events::RouterEventsManager;
use crate::interface::http_ws_listener::ObjectHttpWSService;
use crate::name::NameResolver;
use crate::resolver::OodResolver;
use crate::root_state_api::*;
use crate::router_handler::RouterHandlersManager;
use crate::stack::ObjectServices;
use crate::zone::ZoneRoleManager;
use cyfs_base::*;
use cyfs_lib::NONProtocol;
use cyfs_bdt::StackGuard;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

pub struct ObjectListenerManagerParams {
    pub bdt_stack: StackGuard,

    // bdt协议栈监听的vport列表
    pub bdt_listeners: Vec<u16>,

    // tcp协议监听的地址列表
    pub tcp_listeners: Vec<SocketAddr>,

    // websocket
    pub ws_listener: Option<SocketAddr>,
}

struct AuthenticatedServerInfo {
    gateway_ip: String,
    listener: Box<dyn ObjectListener>,
    server: HttpServerHandlerRef,

    ws_event_interface: Option<WebSocketEventInterface>,
}

pub struct ObjectListenerManager {
    device_id: DeviceId,

    // interfaces
    listeners: Vec<Box<dyn ObjectListener>>,
    ws_event_interface: Option<WebSocketEventInterface>,

    // http request handler servers
    http_tcp_server: Option<HttpServerHandlerRef>,
    http_bdt_server: Option<HttpServerHandlerRef>,

    // authenticated interface and server
    router_handlers_manager: Option<RouterHandlersManager>,
    router_events_manager: Option<RouterEventsManager>,
    http_auth_raw_server: Option<HttpServerHandlerRef>,
    authenticated_server: Mutex<Option<AuthenticatedServerInfo>>,

    url_translator: Option<UrlTransaltor>,
    default_handler: Option<HttpDefaultHandler>,
}

pub type ObjectListenerManagerRef = Arc<ObjectListenerManager>;

impl ObjectListenerManager {
    pub fn new(device_id: DeviceId) -> Self {
        Self {
            device_id,
            listeners: Vec::new(),
            ws_event_interface: None,
            http_tcp_server: None,
            http_bdt_server: None,

            router_handlers_manager: None,
            router_events_manager: None,
            http_auth_raw_server: None,
            authenticated_server: Mutex::new(None),
            url_translator: None,
            default_handler: None,
        }
    }

    pub fn listeners_count(&self) -> usize {
        self.listeners.len()
    }

    // 获取一个可用的http服务地址
    pub fn get_available_http_listener(&self) -> Option<SocketAddr> {
        use cyfs_lib::*;
        for listener in &self.listeners {
            if listener.get_protocol() == NONProtocol::HttpLocal {
                return Some(listener.get_addr());
            }
        }

        None
    }

    // 获取ws的服务地址，当前stack必须已经开启ws event服务
    pub fn get_ws_event_listener(&self) -> Option<SocketAddr> {
        if let Some(ws_event_interface) = &self.ws_event_interface {
            Some(ws_event_interface.get_ws_event_listener())
        } else {
            warn!("ws event interface not exists!");
            None
        }
    }

    pub fn get_http_tcp_server(&self) -> HttpServerHandlerRef {
        self.http_tcp_server.as_ref().unwrap().clone()
    }

    pub fn get_http_bdt_server(&self) -> HttpServerHandlerRef {
        self.http_bdt_server.as_ref().unwrap().clone()
    }

    pub(crate) fn init(
        &mut self,
        params: ObjectListenerManagerParams,
        services: &ObjectServices,
        router_handlers: &RouterHandlersManager,
        router_events: &RouterEventsManager,
        name_resolver: &NameResolver,
        acl: &AclManagerRef,
        app_service: &AppService,
        role_manager: &ZoneRoleManager,
        root_state: &GlobalStateService,
        local_cache: &GlobalStateLocalService,
        ood_resolver: OodResolver,
    ) {
        assert!(self.listeners.is_empty());

        let url_translator = UrlTransaltor::new(
            name_resolver.clone(),
            app_service.clone(),
            role_manager.zone_manager().clone(),
            ood_resolver,
        );
        let default_handler = HttpDefaultHandler::default();

        // 首先初始化三个基础的http_server
        {
            assert!(self.http_bdt_server.is_none());
            let server = ObjectHttpListener::new(
                NONProtocol::HttpBdt,
                services,
                router_handlers,
                acl.clone(),
                role_manager.sync_server().clone(),
                role_manager.sync_client().clone(),
                root_state,
                local_cache,
            );

            let raw_handler = RawHttpServer::new(server.into_server());
            let http_server = DefaultHttpServer::new(
                raw_handler.into(),
                Some(url_translator.clone()),
                default_handler.clone(),
            );
            self.http_bdt_server = Some(http_server.into());
        }

        {
            assert!(self.http_tcp_server.is_none());
            let server = ObjectHttpListener::new(
                NONProtocol::HttpLocal,
                services,
                router_handlers,
                acl.clone(),
                role_manager.sync_server().clone(),
                role_manager.sync_client().clone(),
                root_state,
                local_cache,
            );

            let raw_handler = RawHttpServer::new(server.into_server());
            let http_server = DefaultHttpServer::new(
                raw_handler.into(),
                Some(url_translator.clone()),
                default_handler.clone(),
            );
            self.http_tcp_server = Some(http_server.into());
        }

        {
            assert!(self.http_auth_raw_server.is_none());
            let server = ObjectHttpListener::new(
                NONProtocol::HttpLocalAuth,
                services,
                router_handlers,
                acl.clone(),
                role_manager.sync_server().clone(),
                role_manager.sync_client().clone(),
                root_state,
                local_cache,
            );

            let raw_handler = RawHttpServer::new(server.into_server());
            
            self.http_auth_raw_server = Some(raw_handler.into());
        }

        // save url_translator and default_handler for dynamic auth interface
        assert!(self.url_translator.is_none());
        assert!(self.default_handler.is_none());
        self.url_translator = Some(url_translator);
        self.default_handler = Some(default_handler);

        // init all listeners
        for vport in params.bdt_listeners {
            let http_server = self.http_bdt_server.as_ref().unwrap().clone();

            let bdt_listener =
                ObjectHttpBdtListener::new(params.bdt_stack.clone(), vport, http_server);
            let bdt_listener = Box::new(bdt_listener) as Box<dyn ObjectListener>;
            self.listeners.push(bdt_listener);
        }

        for addr in params.tcp_listeners {
            let http_server = self.http_tcp_server.as_ref().unwrap().clone();

            let tcp_listener =
                ObjectHttpTcpListener::new(addr, self.device_id.clone(), http_server);
            let tcp_listener = Box::new(tcp_listener) as Box<dyn ObjectListener>;
            self.listeners.push(tcp_listener);
        }

        // ws interface
        assert!(self.ws_event_interface.is_none());
        if let Some(addr) = params.ws_listener {
            let http_server = self.http_tcp_server.as_ref().unwrap().clone();
            let http_ws_service =
                ObjectHttpWSService::new(addr.clone(), self.device_id.clone(), http_server);

            let ws_event_interface = WebSocketEventInterface::new(
                http_ws_service,
                router_handlers.clone(),
                router_events.clone(),
                addr,
            );
            self.ws_event_interface = Some(ws_event_interface);
            self.router_events_manager = Some(router_events.clone());
            self.router_handlers_manager = Some(router_handlers.clone());
        }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        for listener in &self.listeners {
            if let Err(e) = listener.start().await {
                error!(
                    "start object listener error: addr={}, {}",
                    listener.get_addr(),
                    e
                );
                return Err(e);
            }
        }

        if let Some(ws_event_interface) = &self.ws_event_interface {
            ws_event_interface.start().await.map_err(|e| {
                error!(
                    "start ws event interface error: addr={}, {}",
                    ws_event_interface.get_ws_event_listener(),
                    e
                );
                e
            })?;
        }

        Ok(())
    }

    pub async fn restart(&self) -> BuckyResult<()> {
        for listener in &self.listeners {
            if let Err(e) = listener.restart().await {
                error!(
                    "restart object listener error: addr={}, {}",
                    listener.get_addr(),
                    e
                );
                return Err(e);
            }
        }

        if let Some(ws_event_interface) = &self.ws_event_interface {
            ws_event_interface.restart().await.map_err(|e| {
                error!(
                    "restart ws event interface error: addr={}, {}",
                    ws_event_interface.get_ws_event_listener(),
                    e
                );
                e
            })?;
        }

        /*
        // 网卡变动时候重启所有interface，应该不包括sanbox的gateway interface
        {
            let auth_server = self.authenticated_server.lock().unwrap();
            if let Some(auth_server) = *auth_server {
                auth_server.listener.restart().await.map_err(|e| {
                    error!(
                        "restart authenticated_interface error: addr={}, {}",
                        auth_server.addr(),
                        e
                    );
                    e
                })?;
            }
        }
        */

        Ok(())
    }

    pub(crate) async fn start_authenticated_interface(
        &self,
        gateway_ip: &str,
        auth_app_list: AuthenticatedAppList,
    ) -> BuckyResult<()> {
        // check if running already
        let mut should_stop = false;
        {
            let slot = self.authenticated_server.lock().unwrap();
            if let Some(auth_server) = &*slot {
                if auth_server.gateway_ip == gateway_ip {
                    return Ok(());
                } else {
                    should_stop = true;
                }
            }
        }

        // first stop running interface
        if should_stop {
            let _ = self.stop_authenticated_interface().await;
        }

        let raw_http_server = self.http_auth_raw_server.as_ref().unwrap().clone();
        let auth_http_server = AuthenticatedHttpServer::new(raw_http_server.into(), auth_app_list);
        let server = DefaultHttpServer::new(
            auth_http_server.into(),
            Some(self.url_translator.as_ref().unwrap().clone()),
            self.default_handler.as_ref().unwrap().clone(),
        ).into();

        let addr = format!("{}:{}", gateway_ip, 0);
        let mut sock_addr: SocketAddr = addr.parse().map_err(|e| {
            let msg = format!("invalid authenticated gateway addr: {}, {}", addr, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        info!("will start authenticated interface: addr={}", gateway_ip);

        // http interface
        sock_addr.set_port(NON_STACK_HTTP_PORT);
        let listener =
            ObjectHttpTcpListener::new(sock_addr.clone(), self.device_id.clone(), server.clone());

        let listener = Box::new(listener) as Box<dyn ObjectListener>;
        listener.start().await?;

        // ws interface
        let ws_event_interface = if self.ws_event_interface.is_some() {
            sock_addr.set_port(NON_STACK_WS_PORT);
            let http_ws_service =
                ObjectHttpWSService::new(sock_addr.clone(), self.device_id.clone(), server.clone());

            let ws_event_interface = WebSocketEventInterface::new(
                http_ws_service,
                self.router_handlers_manager.as_ref().unwrap().clone(),
                self.router_events_manager.as_ref().unwrap().clone(),
                sock_addr,
            );

            if let Err(e) = ws_event_interface.start().await {
                let _ = listener.stop().await;
                return Err(e);
            }

            Some(ws_event_interface)
        } else {
            None
        };

        let info = AuthenticatedServerInfo {
            gateway_ip: gateway_ip.to_owned(),
            listener,
            server,
            ws_event_interface,
        };

        {
            let mut slot = self.authenticated_server.lock().unwrap();
            assert!(slot.is_none());
            *slot = Some(info);
        }

        Ok(())
    }

    pub async fn stop_authenticated_interface(&self) -> BuckyResult<()> {
        let info = self.authenticated_server.lock().unwrap().take();
        if let Some(auth_server) = info {
            info!(
                "will stop authenticated interface at {}",
                auth_server.gateway_ip
            );
            auth_server.listener.stop().await?;

            if let Some(interface) = auth_server.ws_event_interface {
                interface.stop().await;
            }
        }

        Ok(())
    }
}
