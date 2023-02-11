use std::str::FromStr;
use tide::http::Url;
use async_std::net::{SocketAddr, ToSocketAddrs, TcpStream};
use http_types::{
    Request, Method, StatusCode, Mime,
};

use crate::panic::BugReportHandler;
use cyfs_base::*;
use crate::panic::CyfsPanicInfo;
use super::request::PanicReportRequest;


// 默认的addr
// const NOTIFY_ADDR: &str = "http://127.0.0.1:40001/bugs/";


#[derive(Clone)]
pub struct HttpBugReporter {
    notify_addr: Url,
}

impl HttpBugReporter {
    pub fn new(addr: &str) -> Self {
        info!("new http bug reporter: {}", addr);

        let url = Url::from_str(addr).unwrap();
        Self {
            notify_addr: url,
        }
    }

    pub async fn notify(&self, req: PanicReportRequest) -> BuckyResult<()> {
        let s = req.to_string();

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

impl BugReportHandler for HttpBugReporter {
    fn notify(&self, product_name: &str, service_name: &str, panic_info: &CyfsPanicInfo) -> BuckyResult<()> {
        let req = PanicReportRequest::new(product_name, service_name, panic_info.to_owned());
        let this = self.clone();
        async_std::task::block_on(async move {
            let _ = this.notify(req).await;
        });

        Ok(())
    }
}
