use super::super::protocol::*;
use super::device_sync_client::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;
use tide::Response;

#[derive(Clone)]
pub(crate) struct DeviceSyncRequestHandler {
    protocol: NONProtocol,
    client: Arc<DeviceSyncClient>,
}

impl DeviceSyncRequestHandler {
    pub fn new(protocol: NONProtocol, client: Arc<DeviceSyncClient>) -> Self {
        Self { protocol, client }
    }

    pub async fn process_zone_request<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> Response {
        let ret = self.on_zone(req, body).await;
        match ret {
            Ok(_) => {
                let http_resp: Response = RequestorHelper::new_ok_response();

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_state_request<State>(&self, req: tide::Request<State>) -> Response {
       
        let ret = self.on_get_sync_state(req).await;
        match ret {
            Ok(status) => {
                let mut http_resp: Response = RequestorHelper::new_ok_response();

                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(status.encode_string());
                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_zone<State>(&self, req: tide::Request<State>, body: String) -> BuckyResult<()> {
        let zone_req = SyncZoneRequest::decode_string(&body)?;

        // 提取来源device
        let req: http_types::Request = req.into();
        let source = RequestorHelper::decode_header(&req, ::cyfs_base::CYFS_REMOTE_DEVICE)?;

        self.client.zone_update(source, zone_req).await
    }

    async fn on_get_sync_state<State>(&self, req: tide::Request<State>) -> BuckyResult<DeviceSyncStatus> {
        let flush = match req.method() {
            http_types::Method::Post => true,
            http_types::Method::Get => false,
            _ => unreachable!(),
        };

        // 提取来源device
        let req: http_types::Request = req.into();
        let source = RequestorHelper::decode_header(&req, ::cyfs_base::CYFS_REMOTE_DEVICE)?;

        self.client.get_sync_state(source, flush).await
    }
}
