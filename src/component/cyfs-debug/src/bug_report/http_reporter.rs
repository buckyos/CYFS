use std::str::FromStr;
use tide::http::Url;
use http_types::{
    StatusCode,
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
        self.post(req).await
    }

    async fn post(&self, req: PanicReportRequest) -> BuckyResult<()> {
        let report_url = self.notify_addr.join(&req.info.hash).unwrap();

        let mut resp = surf::post(report_url).body_json(&req)?.await?;
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
