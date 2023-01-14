use super::request::*;
use crate::*;
use cyfs_base::BuckyResult;

use http_types::{Method, Request, StatusCode, Url};
use std::sync::Arc;


pub struct SyncRequestor {
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl SyncRequestor {
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
            StatusCode::Ok => {
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
