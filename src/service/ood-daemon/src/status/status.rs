use cyfs_base::*;
use cyfs_lib::RequestorHelper;
use ood_control::*;
use super::service_status::*;

use serde::Serialize;
use std::sync::{Arc, Mutex};
use once_cell::sync::OnceCell;


#[derive(Serialize)]
struct ServiceStatusCache {
    name: String,
    status: serde_json::Value,
    last_update_tick: u64,
}

#[derive(Clone)]
pub struct OODStatusManager {
    service_list: Arc<Mutex<Vec<ServiceStatusCache>>>,
    interface: Arc<OnceCell<HttpTcpListener>>,
}

impl OODStatusManager {
    fn new() -> Self {
        let ret = Self {
            service_list: Arc::new(Mutex::new(vec![])),
            interface: Arc::new(OnceCell::new()),
        };

        ret.init_interface();
        ret
    }

    fn init_interface(&self) {
        let mut server = HttpServer::new_server();
        self.register(&mut server);

        let addr = std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            OOD_DAEMON_LOCAL_STATUS_PORT,
        );

        let interface = HttpTcpListener::new_with_raw_server(addr, Arc::new(server));
        if let Err(_) = self.interface.set(interface) {
            unreachable!();
        }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        self.interface.get().unwrap().start().await
    }

    pub fn register_server(&self) {
        OOD_CONTROLLER.register_external_server(Box::new(self.clone()));
    }

    pub fn update_ood_daemon_status(&self, status: &OODDaemonStatus) {
        let value = serde_json::to_value(&status).unwrap();
        self.update_service("ood-daemon", value);
    }

    fn get_all(&self) -> String {
        let ret = serde_json::to_string(&*self.service_list.lock().unwrap()).unwrap();

        ret
    }

    fn get_one(&self, name: &str) -> Option<String> {
        let list = self.service_list.lock().unwrap();
        for item in list.iter() {
            if item.name == name {
                let ret = serde_json::to_string(item).unwrap();
                return Some(ret);
            }
        }

        None
    }

    fn update_service(&self, name: &str, status: serde_json::Value) {
        let mut list = self.service_list.lock().unwrap();
        for item in list.iter_mut() {
            if item.name == name {
                item.status = status;
                item.last_update_tick = bucky_time_now();
                return;
            }
        }

        list.push(ServiceStatusCache {
            name: name.to_owned(),
            status,
            last_update_tick: bucky_time_now(),
        });
    }
}

enum RequestType {
    GetStatus,
    ReportStatus,
}

pub(crate) struct HttpServerEndpoint {
    req_type: RequestType,
    handler: OODStatusManager,
}

impl HttpServerEndpoint {
    async fn on_get_status<State>(&self, req: tide::Request<State>) -> tide::Response {
        let name = match req.param("name") {
            Ok(v) => Some(v.to_owned()),
            Err(_) => None,
        };

        let ret = match name {
            Some(name) => match self.handler.get_one(&name) {
                Some(v) => v,
                None => {
                    let msg = format!("service not found: {}", name);
                    warn!("{}", msg);
                    let err = BuckyError::new(BuckyErrorCode::NotFound, msg);
                    return RequestorHelper::trans_error(err);
                }
            },
            None => self.handler.get_all(),
        };

        let mut resp: tide::Response = RequestorHelper::new_ok_response();
        resp.set_content_type(tide::http::mime::JSON);
        resp.set_body(ret);
        resp
    }

    async fn process_report_status<State>(&self, req: tide::Request<State>) -> tide::Response {
        let ret = self.on_report_status(req).await;
        match ret {
            Ok(_) => RequestorHelper::new_ok_response(),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_report_status<State>(&self, mut req: tide::Request<State>) -> BuckyResult<()> {
        let name = req
            .param("name")
            .map_err(|e| {
                let msg = format!("invalid service name: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?
            .to_owned();

        let value = req.body_string().await.map_err(|e| {
            let msg = format!("read service status value failed: name={}, {}", name, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        info!(
            "recv server status report: service={}, valuje={}",
            name, value
        );

        let value: serde_json::Value = serde_json::from_str(&value).map_err(|e| {
            let msg = format!(
                "invalid service status json format: name={}, value={}, {}",
                name, value, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        self.handler.update_service(&name, value);

        Ok(())
    }
}

#[async_trait::async_trait]
impl<State> tide::Endpoint<State> for HttpServerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: tide::Request<State>) -> tide::Result {
        let resp = match self.req_type {
            RequestType::GetStatus => self.on_get_status(req).await,
            RequestType::ReportStatus => self.process_report_status(req).await,
        };

        Ok(resp)
    }
}

impl ExternalServerEndPoint for OODStatusManager {
    fn register(&self, server: &mut ::tide::Server<()>) {
        server.at("/service_status").get(HttpServerEndpoint {
            req_type: RequestType::GetStatus,
            handler: self.clone(),
        });
        server.at("/service_status/:name").get(HttpServerEndpoint {
            req_type: RequestType::GetStatus,
            handler: self.clone(),
        });

        server
            .at("/service_status/:name")
            .post(HttpServerEndpoint {
                req_type: RequestType::ReportStatus,
                handler: self.clone(),
            });
    }
}


lazy_static::lazy_static! {
    pub static ref OOD_STATUS_MANAGER: OODStatusManager = OODStatusManager::new();
}