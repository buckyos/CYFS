use super::request::*;
use super::processor::*;
use crate::{base::*, SharedObjectStackDecID, UtilBuildDirFromObjectMapOutputRequest, UtilBuildDirFromObjectMapOutputResponse, UtilBuildFileOutputRequest, UtilBuildFileOutputResponse};
use cyfs_base::*;

use cyfs_core::{Zone, ZoneId};
use http_types::{Method, Request, Response, Url};
use std::sync::Arc;

#[derive(Clone)]
pub struct UtilRequestor {
    dec_id: Option<SharedObjectStackDecID>,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl UtilRequestor {
    pub fn new_default_tcp(dec_id: Option<SharedObjectStackDecID>) -> Self {
        let service_addr = format!("127.0.0.1:{}", cyfs_base::NON_STACK_HTTP_PORT);
        Self::new_tcp(dec_id, &service_addr)
    }

    pub fn new_tcp(dec_id: Option<SharedObjectStackDecID>, service_addr: &str) -> Self {
        let tcp_requestor = TcpHttpRequestor::new(service_addr);
        Self::new(dec_id, Arc::new(Box::new(tcp_requestor)))
    }

    pub fn new(dec_id: Option<SharedObjectStackDecID>, requestor: HttpRequestorRef) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/util/", addr);
        let url = Url::parse(&url).unwrap();

        Self {
            dec_id,
            requestor,
            service_url: url,
        }
    }

    pub fn into_processor(self) -> UtilOutputProcessorRef {
        Arc::new(Box::new(self))
    }

    pub fn clone_processor(&self) -> UtilOutputProcessorRef {
        self.clone().into_processor()
    }

    // url支持下面的格式，其中device_id是可选
    // {host:port}/util/{util_path}/[req_path/]object_id[/inner_path]
    fn format_url(
        &self,
        util_path: &str,
        req_path: Option<&String>,
        object_id: Option<&ObjectId>,
    ) -> Url {
        let mut parts = vec![];
        parts.push(util_path);

        if let Some(req_path) = req_path {
            parts.push(
                req_path
                    .as_str()
                    .trim_start_matches('/')
                    .trim_end_matches('/'),
            );
        }

        let object_id = object_id.map(|v| v.to_string());
        if let Some(object_id) = &object_id {
            parts.push(object_id.as_str());
        }
        let p = parts.join("/");
        self.service_url.join(&p).unwrap()
    }

    fn encode_common_headers(&self, com_req: &UtilRequestCommon, http_req: &mut Request) {
        if let Some(dec_id) = &com_req.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        } else if let Some(dec_id) = &self.dec_id {
            if let Some(dec_id) = dec_id.get() {
                http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
            }
        }

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());
    }

    fn encode_get_device_request(&self, req: &UtilGetDeviceRequest) -> Request {
        let url = self.format_url("device", req.common.req_path.as_ref(), None);
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    async fn decode_get_device_response(
        mut resp: Response,
    ) -> BuckyResult<UtilGetDeviceResponse> {
        let buf = resp.body_bytes().await.map_err(|e| {
            let msg = format!("get_current_device failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        let (device, _) = Device::raw_decode(&buf).map_err(|e| {
            error!("decode device from resp bytes error: {}", e);
            e
        })?;

        let device_id: DeviceId = device.desc().device_id().clone();

        Ok(UtilGetDeviceResponse { device_id, device })
    }

    // xxx/util/device
    pub async fn get_device(
        &self,
        req: UtilGetDeviceRequest,
    ) -> BuckyResult<UtilGetDeviceResponse> {
        let http_req = self.encode_get_device_request(&req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = Self::decode_get_device_response(resp).await?;
            info!("util get_device from non stack success: {}", resp);

            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            Err(e)
        }
    }

    fn encode_get_zone_request(&self, req: UtilGetZoneRequest) -> Request {
        let url = self.format_url("zone", req.common.req_path.as_ref(), req.object_id.as_ref());
        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(&req.common, &mut http_req);

        if let Some(object_raw) = req.object_raw {
            http_req.set_body(object_raw);
        }

        http_req
    }
    async fn decode_get_zone_response(
        mut resp: Response,
    ) -> BuckyResult<UtilGetZoneResponse> {
        let zone: Zone = RequestorHelper::decode_raw_object_body(&mut resp).await?;
        let zone_id: ZoneId = RequestorHelper::decode_header(&resp, cyfs_base::CYFS_ZONE_ID)?;

        let device_id = RequestorHelper::decode_header(&resp, cyfs_base::CYFS_OOD_DEVICE_ID)?;

        let resp = UtilGetZoneResponse {
            zone,
            zone_id,
            device_id,
        };

        info!("util get_zone from non stack success: {}", resp);

        Ok(resp)
    }

    // 根据device/people/simplegroup查询所在的zone
    // 如果已知object的内容，那么可以附带，加速non-stack的查询
    // xxx/util/zone[/object_id]
    pub async fn get_zone(
        &self,
        req: UtilGetZoneRequest,
    ) -> BuckyResult<UtilGetZoneResponse> {
        let http_req = self.encode_get_zone_request(req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = Self::decode_get_zone_response(resp).await?;

            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            Err(e)
        }
    }

    // url支持下面的格式，其中owner_id是可选
    // {host:port}/[req_path/]object_id?owner={owner_id}
    fn format_resolve_url(&self,  req_path: Option<&String>, owner_id: Option<&ObjectId>, object_id: &ObjectId) -> Url {
        let mut url = self.format_url("resolve_ood", req_path, Some(object_id));

        if let Some(owner_id) = owner_id {
            url.query_pairs_mut().append_pair("owner", &owner_id.to_string());
        }

        url
    }

    fn encode_resolve_ood_request(&self, req: &UtilResolveOODRequest) -> Request {
        let url = self.format_resolve_url(req.common.req_path.as_ref(), req.owner_id.as_ref(), &req.object_id);

        // 目前没有body
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    pub async fn resolve_ood(
        &self,
        req: UtilResolveOODRequest,
    ) -> BuckyResult<UtilResolveOODResponse> {
        let http_req = self.encode_resolve_ood_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = RequestorHelper::decode_json_body(&mut resp).await?;
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            Err(e)
        }
    }

    fn encode_get_ood_status_request(&self, req: UtilGetOODStatusRequest) -> Request {
        let url = self.format_url("ood_status", req.common.req_path.as_ref(), None);
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    pub async fn get_ood_status(
        &self,
        req: UtilGetOODStatusRequest,
    ) -> BuckyResult<UtilGetOODStatusResponse> {
        let http_req = self.encode_get_ood_status_request(req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = resp.body_json().await.map_err(|e| {
                let msg = format!("parse get_ood_status resp body error! err={}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "util get_ood_status failed: status={}, {}",
                resp.status(),
                e
            );

            Err(e)
        }
    }

    fn encode_get_noc_info_request(&self, req: UtilGetNOCInfoRequest) -> Request {
        let url = self.format_url("noc_info", req.common.req_path.as_ref(), None);
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    pub async fn get_noc_info(
        &self,
        req: UtilGetNOCInfoRequest,
    ) -> BuckyResult<UtilGetNOCInfoResponse> {
        let http_req = self.encode_get_noc_info_request(req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = resp.body_json().await.map_err(|e| {
                let msg = format!("parse get_noc_stat resp body error! err={}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("util get_noc_stat failed: status={}, {}", resp.status(), e);

            Err(e)
        }
    }

    fn encode_get_network_access_info_request(
        &self,
        req: UtilGetNetworkAccessInfoRequest,
    ) -> Request {
        let url = self.format_url("network_access_info", req.common.req_path.as_ref(), None);
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    pub async fn get_network_access_info(
        &self,
        req: UtilGetNetworkAccessInfoRequest,
    ) -> BuckyResult<UtilGetNetworkAccessInfoResponse> {
        let http_req = self.encode_get_network_access_info_request(req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = RequestorHelper::decode_json_body(&mut resp)
                .await
                .map_err(|e| {
                    let msg = format!("parse get_network_access_info resp body error! err={}", e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "util get_network_access_info failed: status={}, {}",
                resp.status(),
                e
            );

            Err(e)
        }
    }

    fn encode_get_device_static_info_request(
        &self,
        req: UtilGetDeviceStaticInfoRequest,
    ) -> Request {
        let url = self.format_url("device_static_info", req.common.req_path.as_ref(), None);
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    pub async fn get_device_static_info(
        &self,
        req: UtilGetDeviceStaticInfoRequest,
    ) -> BuckyResult<UtilGetDeviceStaticInfoResponse> {
        let http_req = self.encode_get_device_static_info_request(req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let content = RequestorHelper::decode_json_body(&mut resp)
                .await
                .map_err(|e| {
                    let msg = format!("parse get_device_static_info resp body error! err={}", e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

            Ok(content)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "util get_device_static_info failed: status={}, {}",
                resp.status(),
                e
            );

            Err(e)
        }
    }

    fn encode_get_system_info_request(&self, req: UtilGetSystemInfoRequest) -> Request {
        let url = self.format_url("system_info", req.common.req_path.as_ref(), None);
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    pub async fn get_system_info(
        &self,
        req: UtilGetSystemInfoRequest,
    ) -> BuckyResult<UtilGetSystemInfoResponse> {
        let http_req = self.encode_get_system_info_request(req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let content = resp.body_json().await.map_err(|e| {
                let msg = format!("parse get_system_info resp body error! err={}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            Ok(content)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "util get_system_info failed: status={}, {}",
                resp.status(),
                e
            );

            Err(e)
        }
    }

    fn encode_get_version_info_request(&self, req: UtilGetVersionInfoRequest) -> Request {
        let url = self.format_url("version_info", req.common.req_path.as_ref(), None);
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    pub async fn get_version_info(
        &self,
        req: UtilGetVersionInfoRequest,
    ) -> BuckyResult<UtilGetVersionInfoResponse> {
        let http_req = self.encode_get_version_info_request(req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let content = RequestorHelper::decode_json_body(&mut resp)
                .await
                .map_err(|e| {
                    let msg = format!("parse get_version_info resp body error! err={}", e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

            Ok(content)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "util get_version_info failed: status={}, {}",
                resp.status(),
                e
            );

            Err(e)
        }
    }

    pub async fn build_file_object(
        &self,
        req: UtilBuildFileOutputRequest
    ) -> BuckyResult<UtilBuildFileOutputResponse> {
        let url = self.format_url("build_file", req.common.req_path.as_ref(), None);
        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(&req.common, &mut http_req);
        let body = req.encode_string();
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            let content = RequestorHelper::decode_json_body(&mut resp)
                .await
                .map_err(|e| {
                    let msg = format!("parse build file resp body error! err={}", e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

            Ok(content)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "util build_file_object failed: status={}, {}",
                resp.status(),
                e
            );

            Err(e)
        }
    }

    pub async fn build_dir_from_object_map(
        &self,
        req: UtilBuildDirFromObjectMapOutputRequest
    ) -> BuckyResult<UtilBuildDirFromObjectMapOutputResponse> {
        let url = self.format_url("build_dir_from_object_map", req.common.req_path.as_ref(), None);
        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(&req.common, &mut http_req);
        let body = req.encode_string();
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            let content = RequestorHelper::decode_json_body(&mut resp)
                .await
                .map_err(|e| {
                    let msg = format!("parse build dir resp body error! err={}", e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

            Ok(content)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "util build_dir_from_object_map failed: status={}, {}",
                resp.status(),
                e
            );

            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl UtilOutputProcessor for UtilRequestor {
    async fn get_device(
        &self,
        req: UtilGetDeviceRequest,
    ) -> BuckyResult<UtilGetDeviceResponse> {
        Self::get_device(&self, req).await
    }

    async fn get_zone(
        &self,
        req: UtilGetZoneRequest,
    ) -> BuckyResult<UtilGetZoneResponse> {
        Self::get_zone(&self, req).await
    }

    async fn resolve_ood(
        &self,
        req: UtilResolveOODRequest,
    ) -> BuckyResult<UtilResolveOODResponse> {
        Self::resolve_ood(&self, req).await
    }

    async fn get_ood_status(
        &self,
        req: UtilGetOODStatusRequest,
    ) -> BuckyResult<UtilGetOODStatusResponse> {
        Self::get_ood_status(&self, req).await
    }

    async fn get_noc_info(
        &self,
        req: UtilGetNOCInfoRequest,
    ) -> BuckyResult<UtilGetNOCInfoResponse> {
        Self::get_noc_info(&self, req).await
    }

    async fn get_network_access_info(
        &self,
        req: UtilGetNetworkAccessInfoRequest,
    ) -> BuckyResult<UtilGetNetworkAccessInfoResponse> {
        Self::get_network_access_info(&self, req).await
    }

    async fn get_device_static_info(
        &self,
        req: UtilGetDeviceStaticInfoRequest,
    ) -> BuckyResult<UtilGetDeviceStaticInfoResponse> {
        Self::get_device_static_info(&self, req).await
    }

    async fn get_system_info(
        &self,
        req: UtilGetSystemInfoRequest,
    ) -> BuckyResult<UtilGetSystemInfoResponse> {
        Self::get_system_info(&self, req).await
    }

    async fn get_version_info(
        &self,
        req: UtilGetVersionInfoRequest,
    ) -> BuckyResult<UtilGetVersionInfoResponse> {
        Self::get_version_info(&self, req).await
    }

    async fn build_file_object(&self, req: UtilBuildFileOutputRequest) -> BuckyResult<UtilBuildFileOutputResponse> {
        Self::build_file_object(self, req).await
    }

    async fn build_dir_from_object_map(&self, req: UtilBuildDirFromObjectMapOutputRequest)
        -> BuckyResult<UtilBuildDirFromObjectMapOutputResponse> {
        Self::build_dir_from_object_map(self, req).await
    }
}
