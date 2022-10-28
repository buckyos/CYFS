use super::def::*;
use super::output_request::*;
use super::processor::*;
use crate::base::*;
use crate::stack::SharedObjectStackDecID;
use cyfs_base::*;

use http_types::{Method, Request, Response, Url};
use std::sync::Arc;

pub struct NONRequestorHelper;

impl NONRequestorHelper {
    async fn decode_object_info_from_body<T>(
        object_id: ObjectId,
        req: &mut T,
    ) -> BuckyResult<NONObjectInfo>
    where
        T: BodyOp + HeaderOp,
    {
        let object_raw = req.body_bytes().await.map_err(|e| {
            let msg = format!(
                "read object bytes request/response error! obj={} {}",
                object_id, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let info = NONObjectInfo::new(object_id, object_raw, None);

        Ok(info)
    }

    pub async fn decode_object_info<T>(req: &mut T) -> BuckyResult<NONObjectInfo>
    where
        T: BodyOp + HeaderOp,
    {
        // 头部必须有object-id字段
        let object_id: ObjectId = RequestorHelper::decode_header(req, cyfs_base::CYFS_OBJECT_ID)?;

        let mut info = Self::decode_object_info_from_body(object_id, req).await?;
        info.decode_and_verify()?;
        Ok(info)
    }

    pub async fn decode_allow_empty_object_info<T>(req: &mut T) -> BuckyResult<NONObjectInfo>
    where
        T: BodyOp + HeaderOp,
    {
        // 头部必须有object-id字段
        let object_id: ObjectId = RequestorHelper::decode_header(req, cyfs_base::CYFS_OBJECT_ID)?;

        let mut info = Self::decode_object_info_from_body(object_id, req).await?;
        if !info.is_empty() {
            info.decode_and_verify()?;
        }
        Ok(info)
    }

    pub async fn decode_option_object_info<T>(req: &mut T) -> BuckyResult<Option<NONObjectInfo>>
    where
        T: BodyOp + HeaderOp,
    {
        // 头部必须有object-id字段
        let ret: Option<ObjectId> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_OBJECT_ID)?;
        if ret.is_none() {
            return Ok(None);
        }

        let mut info = Self::decode_object_info_from_body(ret.unwrap(), req).await?;
        info.decode_and_verify()?;

        Ok(Some(info))
    }

    pub fn encode_object_info<T>(req: &mut T, info: NONObjectInfo)
    where
        T: BodyOp + HeaderOp,
    {
        req.insert_header(cyfs_base::CYFS_OBJECT_ID, info.object_id.to_string());

        if info.object_raw.len() > 0 {
            req.set_body(info.object_raw);
            req.set_content_type(CYFS_OBJECT_MIME.clone());
        }
    }

    pub async fn decode_get_object_response<T>(
        resp: &mut T,
    ) -> BuckyResult<NONGetObjectOutputResponse>
    where
        T: BodyOp + HeaderOp,
    {
        let object = Self::decode_object_info(resp).await?;
        let attr: Option<u32> =
            RequestorHelper::decode_optional_header(resp, cyfs_base::CYFS_ATTRIBUTES)?;
        let attr = attr.map(|v| Attributes::new(v));

        let object_update_time =
            RequestorHelper::decode_optional_header(resp, cyfs_base::CYFS_OBJECT_UPDATE_TIME)?;
        let object_expires_time =
            RequestorHelper::decode_optional_header(resp, cyfs_base::CYFS_OBJECT_EXPIRES_TIME)?;

        let ret = NONGetObjectOutputResponse {
            object,
            object_expires_time,
            object_update_time,
            attr,
        };

        Ok(ret)
    }
}

#[derive(Clone)]
pub struct NONRequestor {
    dec_id: Option<SharedObjectStackDecID>,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl NONRequestor {
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

        let url = format!("http://{}/non/", addr);
        let url = Url::parse(&url).unwrap();

        let ret = Self {
            dec_id,
            requestor,
            service_url: url,
        };

        ret
    }

    pub fn into_processor(self) -> NONOutputProcessorRef {
        Arc::new(Box::new(self))
    }

    pub fn clone_processor(&self) -> NONOutputProcessorRef {
        self.clone().into_processor()
    }

    fn encode_common_headers(
        &self,
        action: NONAction,
        com_req: &NONOutputRequestCommon,
        http_req: &mut Request,
    ) {
        if let Some(dec_id) = &com_req.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        } else if let Some(dec_id) = &self.dec_id {
            if let Some(dec_id) = dec_id.get() {
                http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
            }
        }

        RequestorHelper::encode_opt_header_with_encoding(
            http_req,
            cyfs_base::CYFS_REQ_PATH,
            com_req.req_path.as_deref(),
        );

        http_req.insert_header(cyfs_base::CYFS_NON_ACTION, action.to_string());

        http_req.insert_header(cyfs_base::CYFS_API_LEVEL, com_req.level.to_string());

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        if let Some(source) = &com_req.source {
            http_req.insert_header(cyfs_base::CYFS_SOURCE, source.to_string());
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());
    }

    fn encode_put_object_request(&self, req: &NONPutObjectOutputRequest) -> Request {
        #[cfg(debug_assertions)]
        {
            if !req.object.is_empty() {
                req.object.verify().expect(&format!(
                    "pub object id unmatch: id={}, object={:?}",
                    req.object.object_id,
                    req.object.object_raw.to_hex()
                ));
            }
        }

        let mut http_req = Request::new(Method::Put, self.service_url.clone());
        self.encode_common_headers(NONAction::PutObject, &req.common, &mut http_req);

        if let Some(access) = &req.access {
            http_req.insert_header(cyfs_base::CYFS_ACCESS, access.value().to_string());
        }

        http_req
    }

    async fn decode_put_object_response(
        &self,
        resp: &Response,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        let result: NONPutObjectResult =
            RequestorHelper::decode_header(resp, cyfs_base::CYFS_RESULT)?;
        let object_update_time =
            RequestorHelper::decode_optional_header(resp, cyfs_base::CYFS_OBJECT_UPDATE_TIME)?;
        let object_expires_time =
            RequestorHelper::decode_optional_header(resp, cyfs_base::CYFS_OBJECT_EXPIRES_TIME)?;

        let ret = NONPutObjectOutputResponse {
            result,
            object_expires_time,
            object_update_time,
        };

        Ok(ret)
    }

    pub async fn put_object(
        &self,
        req: NONPutObjectOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        let object_id = req.object.object_id.clone();

        let mut http_req = self.encode_put_object_request(&req);
        NONRequestorHelper::encode_object_info(&mut http_req, req.object);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!("put object to non service success: {}", object_id);
            self.decode_put_object_response(&resp).await
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "put object to non service error! object={}, {}",
                object_id, e
            );
            Err(e)
        }
    }

    pub async fn update_object_meta(
        &self,
        req: NONUpdateObjectMetaOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        let req = NONPutObjectOutputRequest {
            common: req.common,
            object: NONObjectInfo::new(req.object_id, vec![], None),
            access: req.access,
        };

        self.put_object(req).await
    }

    fn encode_get_object_request(&self, req: &NONGetObjectOutputRequest) -> Request {
        let mut http_req = Request::new(Method::Get, self.service_url.clone());
        self.encode_common_headers(NONAction::GetObject, &req.common, &mut http_req);

        http_req.insert_header(cyfs_base::CYFS_OBJECT_ID, req.object_id.to_string());

        if let Some(inner_path) = &req.inner_path {
            http_req.insert_header(cyfs_base::CYFS_INNER_PATH, inner_path);
        }

        http_req
    }

    pub async fn get_object(
        &self,
        req: NONGetObjectOutputRequest,
    ) -> BuckyResult<NONGetObjectOutputResponse> {
        let http_req = self.encode_get_object_request(&req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!(
                "get object from non service success: {}",
                req.object_debug_info()
            );
            NONRequestorHelper::decode_get_object_response(&mut resp).await
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "get object from non service error! object={}, {}",
                req.object_debug_info(),
                e
            );
            Err(e)
        }
    }

    fn encode_post_object_request(&self, req: &NONPostObjectOutputRequest) -> Request {
        let mut http_req = Request::new(Method::Post, self.service_url.clone());
        self.encode_common_headers(NONAction::PostObject, &req.common, &mut http_req);

        http_req
    }

    async fn decode_post_object_response(
        &self,
        resp: &mut Response,
    ) -> BuckyResult<NONPostObjectOutputResponse> {
        let object = NONRequestorHelper::decode_option_object_info(resp).await?;

        let ret = NONPostObjectOutputResponse { object };

        Ok(ret)
    }

    pub async fn post_object(
        &self,
        req: NONPostObjectOutputRequest,
    ) -> BuckyResult<NONPostObjectOutputResponse> {
        let object_id = req.object.object_id.clone();

        let mut http_req = self.encode_post_object_request(&req);
        NONRequestorHelper::encode_object_info(&mut http_req, req.object);

        let mut resp = self.requestor.request(http_req).await?;

        let status = resp.status();
        if status.is_success() {
            match status {
                http_types::StatusCode::NoContent => {
                    let e = RequestorHelper::error_from_resp(&mut resp).await;
                    info!(
                        "post object to non service but empty response! obj={}, {}",
                        object_id, e
                    );
                    Err(e)
                }
                _ => {
                    info!("post object to non service success: {}", object_id);
                    self.decode_post_object_response(&mut resp).await
                }
            }
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "post object to non service error! object={}, {}",
                object_id, e
            );
            Err(e)
        }
    }

    fn format_select_url(&self, req_path: Option<&String>, filter: &SelectFilter) -> Url {
        let mut url = if let Some(req_path) = req_path {
            self.service_url
                .join(req_path.trim_start_matches('/').trim_end_matches('/'))
                .unwrap()
        } else {
            self.service_url.clone()
        };

        // filter以url params形式编码
        SelectFilterUrlCodec::encode(&mut url, filter);

        url
    }

    fn encode_select_request(&self, req: &NONSelectObjectOutputRequest) -> Request {
        let url = self.format_select_url(req.common.req_path.as_ref(), &req.filter);
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(NONAction::SelectObject, &req.common, &mut http_req);

        SelectOptionCodec::encode(&mut http_req, &req.opt);

        http_req
    }

    pub async fn select_object(
        &self,
        req: NONSelectObjectOutputRequest,
    ) -> BuckyResult<NONSelectObjectOutputResponse> {
        let http_req = self.encode_select_request(&req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = SelectResponse::from_respone(resp).await?;
            Ok(NONSelectObjectOutputResponse {
                objects: resp.objects,
            })
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("select object from non failed: {}", e);
            Err(e)
        }
    }

    fn encode_delete_object_request(&self, req: &NONDeleteObjectOutputRequest) -> Request {
        let mut http_req = Request::new(Method::Delete, self.service_url.clone());
        self.encode_common_headers(NONAction::DeleteObject, &req.common, &mut http_req);

        http_req.insert_header(cyfs_base::CYFS_OBJECT_ID, req.object_id.to_string());

        if let Some(inner_path) = &req.inner_path {
            http_req.insert_header(cyfs_base::CYFS_INNER_PATH, inner_path);
        }

        http_req
    }

    async fn decode_delete_object_response(
        &self,
        req: &NONDeleteObjectOutputRequest,
        resp: &mut Response,
    ) -> BuckyResult<NONDeleteObjectOutputResponse> {
        let object = if req.common.flags & CYFS_REQUEST_FLAG_DELETE_WITH_QUERY != 0 {
            let object = NONRequestorHelper::decode_object_info(resp).await?;
            Some(object)
        } else {
            None
        };

        Ok(NONDeleteObjectOutputResponse { object })
    }

    pub async fn delete_object(
        &self,
        req: NONDeleteObjectOutputRequest,
    ) -> BuckyResult<NONDeleteObjectOutputResponse> {
        let http_req = self.encode_delete_object_request(&req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let ret = self.decode_delete_object_response(&req, &mut resp).await?;
            info!("delete object from non service success: {}, obj={:?}", req.object_id, ret.object);
            Ok(ret)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "delete object from non failed: object={}, {}",
                req.object_id, e
            );
            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl NONOutputProcessor for NONRequestor {
    async fn put_object(
        &self,
        req: NONPutObjectOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        self.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectOutputRequest,
    ) -> BuckyResult<NONGetObjectOutputResponse> {
        self.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectOutputRequest,
    ) -> BuckyResult<NONPostObjectOutputResponse> {
        self.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectOutputRequest,
    ) -> BuckyResult<NONSelectObjectOutputResponse> {
        self.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectOutputRequest,
    ) -> BuckyResult<NONDeleteObjectOutputResponse> {
        self.delete_object(req).await
    }
}
