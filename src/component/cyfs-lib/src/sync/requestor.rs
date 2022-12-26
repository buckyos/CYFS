use super::request::*;
use crate::base::*;
use cyfs_base::BuckyResult;

use http_types::{Method, Request, Url};
use std::sync::Arc;


pub struct SyncRequestor {
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl Default for SyncRequestor {
    fn default() -> Self {
        let service_addr = format!("127.0.0.1:{}", cyfs_base::NON_STACK_HTTP_PORT);

        Self::new_tcp(&service_addr)
    }
}

impl SyncRequestor {
    pub fn new_tcp(service_addr: &str) -> Self {
        let tcp_requestor = TcpHttpRequestor::new(service_addr);
        Self::new(Arc::new(Box::new(tcp_requestor)))
    }

    pub fn new(requestor: HttpRequestorRef) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/sync/", addr);
        let url = Url::parse(&url).unwrap();

        Self {
            requestor,
            service_url: url,
        }
    }

    pub async fn sync_status(&self, flush: bool) -> BuckyResult<DeviceSyncStatus> {
        let url = self.service_url.join("status").unwrap();

        let http_req = match flush {
            true => Request::new(Method::Post, url),
            false => Request::new(Method::Get, url),
        };

        debug!("will get device sync status: flush={}", flush);

        let mut resp = self.requestor.request_timeout(http_req, std::time::Duration::from_secs(30)).await?;

        match resp.status() {
            code if code.is_success() => {
                let ret = RequestorHelper::decode_json_body(&mut resp).await?;

                info!(
                    "get device sync status success: flush={}, status={:?}",
                    flush, ret
                );
                Ok(ret)
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;

                error!("get device sync status failed: code={}, err={}", code, e);
                Err(e)
            }
        }
    }
}
