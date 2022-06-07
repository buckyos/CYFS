use std::str::FromStr;
use tide::http::Url;
use async_std::net::{SocketAddr, ToSocketAddrs, TcpStream};
use http_types::{
    Request, Method, StatusCode, Mime,
};
use serde::{Serialize, Deserialize};
use crate::panic::BugReportHandler;

use cyfs_base::*;
use crate::panic::CyfsPanicInfo;

// 从内置环境变量获取一些信息
lazy_static::lazy_static! {
    /// The global buffer pool we use for storing incoming data.
    static ref TARGET: &'static str = get_target();
    static ref VERSION: &'static str = get_version();
}

// 默认的addr
const NOTIFY_ADDR: &str = "http://127.0.0.1:40001/bugs/";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PanicReportRequest {
    pub product_name: String,
    pub service_name: String,
    pub target: String,
    pub exe_name: String,
    pub version: String,

    pub info: CyfsPanicInfo,
}

#[derive(Clone)]
pub struct BugReporter {
    notify_addr: Url,
}

impl Default for BugReporter {
    fn default() -> Self {
        let url = Url::from_str(NOTIFY_ADDR).unwrap();
        Self {
            notify_addr: url,
        }
    }
}

impl BugReporter {
    pub fn new(addr: &str) -> Self {
        let url = Url::from_str(addr).unwrap();
        Self {
            notify_addr: url,
        }
    }

    pub async fn notify(&self, product_name: &str, service_name: &str, panic_info: CyfsPanicInfo) -> BuckyResult<()> {
        let exe_name = match std::env::current_exe() {
            Ok(path) => {
                match path.file_name() {
                    Some(v) => {
                        v.to_str().unwrap_or("[unknown]").to_owned()
                    }
                    None => {
                        "[unknown]".to_owned()
                    }
                }
            }
            Err(_e) => "[unknown]".to_owned(), 
        };

        let req = PanicReportRequest {
            product_name: product_name.to_owned(),
            service_name: service_name.to_owned(),
            target: TARGET.to_owned(),
            exe_name,
            version: VERSION.to_owned(),
            info: panic_info,
        };

        let s = serde_json::to_string(&req).map_err(|e| {
            let msg = format!("encode panic req error: {}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        self.post(&req.info.hash, s).await
    }

    async fn post(&self, hash: &str, msg: String) -> BuckyResult<()> {
        
        let report_url = self.notify_addr.join(hash).unwrap();

        let host = self.notify_addr.host_str().unwrap();
        let port = self.notify_addr.port_or_known_default().unwrap();
        let addr = format!("{}:{}", host, port);

        info!("addr={}", addr);
        let addrs: Vec<SocketAddr> = addr.to_socket_addrs().await.map_err(|e| {
            let msg = format!("resolve dns error: {}, {}", addr, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?.collect();

        assert!(addrs.len() > 0);

        
        let stream = TcpStream::connect(&addrs[0]).await.map_err(|e| {
            let msg = format!("connect to {} error: {}", addr, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;


        let mut req = Request::new(Method::Post, report_url);
        let mime = Mime::from_str("application/json").unwrap();
        req.set_content_type(mime);
        req.set_body(msg);

        
        let mut resp = async_h1::connect(stream, req).await.map_err(|e| {
            let msg = format!("post to notify addr failed! addr={}, error: {}", self.notify_addr, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;
        
        match resp.status() {
            StatusCode::Ok => {
                info!("post to notify addr success");

                Ok(())
            }
            code @ _ => {
                let body = resp.body_string().await;
                let msg = format!("post to notify addr failed! addr={}, status={}, msg={:?}", 
                    self.notify_addr, code, body);
                error!("{}", msg);
                Err(BuckyError::from(msg))
            }
        }
    }
}

impl BugReportHandler for BugReporter {
    fn notify(&self, product_name: &str, service_name: &str, panic_info: &CyfsPanicInfo) -> BuckyResult<()> {
        let product_name = product_name.to_owned();
        let service_name = service_name.to_owned();
        let info = panic_info.clone();
        let this = self.clone();
        async_std::task::spawn(async move {
            let _ = BugReporter::notify(&this, &product_name, &service_name, info).await;
        });

        Ok(())
    }
}
