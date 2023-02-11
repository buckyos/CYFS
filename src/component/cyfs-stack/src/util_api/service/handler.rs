use crate::non::NONInputHttpRequest;
use crate::util::*;
use cyfs_base::*;
use cyfs_lib::*;

use http_types::StatusCode;
use tide::Response;

#[derive(Clone)]
pub(crate) struct UtilRequestHandler {
    processor: UtilInputProcessorRef,
}

impl UtilRequestHandler {
    pub fn new(processor: UtilInputProcessorRef) -> Self {
        Self { processor }
    }

    fn decode_common_headers<State>(
        req: &NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilInputRequestCommon> {
        // req_path
        let req_path =
            RequestorHelper::decode_optional_header_with_utf8_decoding(&req.request, cyfs_base::CYFS_REQ_PATH)?;

        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = UtilInputRequestCommon {
            req_path,
            source: req.source.clone(),
            target,
            flags: flags.unwrap_or(0),
        };

        Ok(ret)
    }

    // get_device
    fn encode_get_device_response(resp: UtilGetDeviceInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.append_header(cyfs_base::CYFS_DEVICE_ID, resp.device_id.to_string());

        http_resp.set_content_type(CYFS_OBJECT_MIME.clone());

        let buf = resp.device.to_vec().unwrap();
        http_resp.set_body(buf);

        http_resp.into()
    }

    // xxx/device
    pub async fn process_get_device<State>(&self, req: NONInputHttpRequest<State>) -> Response {
        let ret = self.on_get_device(req).await;
        match ret {
            Ok(resp) => Self::encode_get_device_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_device<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilGetDeviceInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        let req = UtilGetDeviceInputRequest { common };

        self.processor.get_device(req).await
    }

    // get_zone
    fn encode_get_zone_response(resp: UtilGetZoneInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
        http_resp.append_header(cyfs_base::CYFS_OOD_DEVICE_ID, resp.device_id.to_string());
        http_resp.append_header(cyfs_base::CYFS_ZONE_ID, resp.zone_id.to_string());

        let buf = resp.zone.to_vec().unwrap();
        http_resp.set_body(buf);

        http_resp.into()
    }

    pub async fn process_get_zone<State>(&self, req: NONInputHttpRequest<State>) -> Response {
        let ret = self.on_get_zone(req).await;
        match ret {
            Ok(resp) => Self::encode_get_zone_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_zone<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilGetZoneInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        let mut ret = UtilGetZoneInputRequest {
            common,
            object_id: None,
            object_raw: None,
        };

        let object_id: Option<ObjectId> = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_OBJECT_ID)?;
        if let Some(object_id) = object_id {
            ret.object_id = Some(object_id);

            let object_raw = req.request.body_bytes().await.map_err(|e| {
                let msg = format!(
                    "read object bytes request/response error! obj={} {}",
                    object_id, e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
            if !object_raw.is_empty() {
                ret.object_raw = Some(object_raw);
            }
        }

        self.processor.get_zone(ret).await
    }

    // resolve_ood
    fn encode_resolve_ood_response(resp: UtilResolveOODInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_content_type(::tide::http::mime::JSON);
        http_resp.set_body(resp.encode_string());

        http_resp.into()
    }

    pub async fn process_resolve_ood_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_resolve_ood_request(req).await;
        match ret {
            Ok(resp) => Self::encode_resolve_ood_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_resolve_ood_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilResolveOODInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        let object_id = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_OBJECT_ID)?;
        let owner_id = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_OWNER_ID)?;

        let req = UtilResolveOODInputRequest {
            common,
            object_id,
            owner_id,
        };

        self.processor.resolve_ood(req).await
    }

    // get_ood_status
    fn encode_get_ood_status_response(resp: UtilGetOODStatusInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_content_type(::tide::http::mime::JSON);
        http_resp.set_body(serde_json::to_string(&resp).unwrap());

        http_resp.into()
    }

    pub async fn process_get_ood_status_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_ood_status_request(req).await;
        match ret {
            Ok(resp) => Self::encode_get_ood_status_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_ood_status_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilGetOODStatusInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        let req = UtilGetOODStatusInputRequest { common };

        self.processor.get_ood_status(req).await
    }

    // get_device_static_info
    fn encode_get_device_static_info_response(
        resp: UtilGetDeviceStaticInfoInputResponse,
    ) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_content_type(::tide::http::mime::JSON);
        http_resp.set_body(resp.encode_string());

        http_resp.into()
    }

    pub async fn process_get_device_static_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_device_static_info_request(req).await;
        match ret {
            Ok(resp) => Self::encode_get_device_static_info_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_device_static_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilGetDeviceStaticInfoInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        let req = UtilGetDeviceStaticInfoInputRequest { common };

        self.processor.get_device_static_info(req).await
    }

    // get_system_info
    fn encode_get_system_info_response(resp: UtilGetSystemInfoInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_content_type(::tide::http::mime::JSON);
        http_resp.set_body(serde_json::to_string(&resp).unwrap());

        http_resp.into()
    }

    pub async fn process_get_system_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_system_info_request(req).await;
        match ret {
            Ok(resp) => Self::encode_get_system_info_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_system_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilGetSystemInfoInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        let req = UtilGetSystemInfoInputRequest { common };

        self.processor.get_system_info(req).await
    }

    // get_noc_info
    fn encode_get_noc_info_response(resp: UtilGetNOCInfoInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_content_type(::tide::http::mime::JSON);
        http_resp.set_body(serde_json::to_string(&resp).unwrap());

        http_resp.into()
    }

    pub async fn process_get_noc_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_noc_info_request(req).await;
        match ret {
            Ok(resp) => Self::encode_get_noc_info_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_noc_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilGetNOCInfoInputResponse> {
        let common = Self::decode_common_headers(&req)?;
    
        let req = UtilGetNOCInfoInputRequest { common };

        self.processor.get_noc_info(req).await
    }

    // get_network_access_info
    fn encode_get_network_access_info_response(
        resp: UtilGetNetworkAccessInfoInputResponse,
    ) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_content_type(::tide::http::mime::JSON);
        http_resp.set_body(resp.encode_string());

        http_resp.into()
    }

    pub async fn process_get_network_access_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_network_access_info_request(req).await;
        match ret {
            Ok(resp) => Self::encode_get_network_access_info_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_network_access_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilGetNetworkAccessInfoInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        let req = UtilGetNetworkAccessInfoInputRequest { common };

        self.processor.get_network_access_info(req).await
    }

    // get_version
    fn encode_get_version_info_response(resp: UtilGetVersionInfoInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_content_type(::tide::http::mime::JSON);
        http_resp.set_body(resp.encode_string());

        http_resp.into()
    }

    pub async fn process_get_version_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_version_info_request(req).await;
        match ret {
            Ok(resp) => Self::encode_get_version_info_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_version_info_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilGetVersionInfoInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        let req = UtilGetVersionInfoInputRequest { common };

        self.processor.get_version_info(req).await
    }

    pub async fn process_build_file_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        match self.on_build_file_request(req).await {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(resp.encode_string());

                http_resp.into()
            },
            Err(e) => {
                RequestorHelper::trans_error(e)
            }
        }
    }

    async fn on_build_file_request<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilBuildFileInputResponse> {
        let body = req.request.body_string().await.map_err(|e| {
            let msg = format!("build file failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let out_req = UtilBuildFileOutputRequest::decode_string(body.as_str())?;

        let in_req = UtilBuildFileInputRequest {
            common: UtilInputRequestCommon {
                req_path: out_req.common.req_path,
                source: req.source,
                target: out_req.common.target,
                flags: out_req.common.flags
            },
            local_path: out_req.local_path,
            owner: out_req.owner,
            chunk_size: out_req.chunk_size,
            access: out_req.access,
        };
        self.processor.build_file_object(in_req).await
    }

    pub async fn process_build_dir_from_object_map_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        match self.on_build_dir_from_object_map_request(req).await {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(resp.encode_string());

                http_resp.into()
            },
            Err(e) => {
                RequestorHelper::trans_error(e)
            }
        }
    }

    async fn on_build_dir_from_object_map_request<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<UtilBuildDirFromObjectMapInputResponse> {
        let body = req.request.body_string().await.map_err(|e| {
            let msg = format!("build file failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let out_req = UtilBuildDirFromObjectMapOutputRequest::decode_string(body.as_str())?;

        let in_req = UtilBuildDirFromObjectMapInputRequest {
            common: UtilInputRequestCommon {
                req_path: out_req.common.req_path,
                source: req.source,
                target: out_req.common.target,
                flags: out_req.common.flags
            },

            object_map_id: out_req.object_map_id,
            dir_type: out_req.dir_type,
        };
        self.processor.build_dir_from_object_map(in_req).await
    }
}
