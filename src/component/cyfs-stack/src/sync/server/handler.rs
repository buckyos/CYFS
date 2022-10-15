use super::super::protocol::*;
use super::zone_sync_server::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;
use tide::Response;

#[derive(Clone)]
pub(crate) struct ZoneSyncRequestHandler {
    protocol: RequestProtocol,
    server: Arc<ZoneSyncServer>,
}

impl ZoneSyncRequestHandler {
    pub fn new(protocol: RequestProtocol, server: Arc<ZoneSyncServer>) -> Self {
        Self { protocol, server }
    }

    fn encode_diff_response(resp: SyncDiffResponse) -> Response {
        let mut http_resp: http_types::Response = RequestorHelper::new_ok_response();
        RequestorHelper::encode_header(&mut http_resp, cyfs_base::CYFS_REVISION, &resp.revision);
        RequestorHelper::encode_opt_header(&mut http_resp, cyfs_base::CYFS_TARGET, &resp.target);

        if resp.objects.len() > 0 {
            if let Err(e)  = SyncObjectsResponse::encode_objects(&mut http_resp, resp.objects) {
                error!("encode diff response error! {}", e);
                return RequestorHelper::trans_error(e);
            }
        }

        http_resp.into()
    }

    pub async fn process_ping_request<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> Response {
        let ret = self.on_ping(req, body).await;
        match ret {
            Ok(resp) => {
                let mut http_resp: Response = RequestorHelper::new_ok_response();

                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(resp.encode_string());

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_diff_request<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> Response {
        let ret = self.on_diff(req, body).await;
        match ret {
            Ok(resp) => Self::encode_diff_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_objects_request<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> Response {
        let ret = self.on_objects(req, body).await;
        match ret {
            Ok(resp) => match resp.into_resonse() {
                Ok(resp) => resp.into(),
                Err(e) => RequestorHelper::trans_error(e),
            },
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_chunks_request<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> Response {
        let ret = self.on_chunks(req, body).await;
        match ret {
            Ok(resp) => {
                let mut http_resp: Response = RequestorHelper::new_ok_response();

                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(resp.encode_string());

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_ping<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> BuckyResult<SyncPingResponse> {
        let ping_req = SyncPingRequest::decode_string(&body)?;

        // 提取来源device
        let req: http_types::Request = req.into();
        let source = RequestorHelper::decode_header(&req, ::cyfs_base::CYFS_REMOTE_DEVICE)?;

        self.server.device_ping(source, ping_req).await
    }

    async fn on_diff<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> BuckyResult<SyncDiffResponse> {
        let sync_diff_req = SyncDiffRequest::decode_string(&body)?;

        // 提取来源device
        let req: http_types::Request = req.into();
        let source = RequestorHelper::decode_header(&req, ::cyfs_base::CYFS_REMOTE_DEVICE)?;

        self.server.sync_diff(source, sync_diff_req).await
    }

    async fn on_objects<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> BuckyResult<SyncObjectsResponse> {
        let objects_req = SyncObjectsRequest::decode_string(&body)?;

        // 提取来源device
        let req: http_types::Request = req.into();
        let source = RequestorHelper::decode_header(&req, ::cyfs_base::CYFS_REMOTE_DEVICE)?;

        self.server.objects(source, objects_req).await
    }

    async fn on_chunks<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> BuckyResult<SyncChunksResponse> {
        let chunks_req = SyncChunksRequest::decode_string(&body)?;

        // 提取来源device
        let req: http_types::Request = req.into();
        let source = RequestorHelper::decode_header(&req, ::cyfs_base::CYFS_REMOTE_DEVICE)?;

        self.server.chunks(source, chunks_req).await
    }
}
