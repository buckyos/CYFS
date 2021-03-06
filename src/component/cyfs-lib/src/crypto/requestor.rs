use super::output_request::*;
use super::processor::*;
use super::request::*;
use crate::non::NONObjectInfo;
use crate::{base::*, SharedObjectStackDecID};
use cyfs_base::*;

use http_types::{Method, Request, Response, Url};
use std::sync::Arc;

#[derive(Clone)]
pub struct CryptoRequestor {
    dec_id: Option<SharedObjectStackDecID>,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl CryptoRequestor {
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

        let url = format!("http://{}/crypto/", addr);
        let url = Url::parse(&url).unwrap();

        Self {
            dec_id,
            requestor,
            service_url: url,
        }
    }

    pub fn into_processor(self) -> CryptoOutputProcessorRef {
        Arc::new(Box::new(self))
    }

    pub fn clone_processor(&self) -> CryptoOutputProcessorRef {
        self.clone().into_processor()
    }

    // url支持下面的格式，其中device_id是可选
    // {host:port}/crypto/verify|sign/[req_path/]object_id
    fn format_url(&self, sign: bool, req_path: Option<&String>, object_id: &ObjectId) -> Url {
        let mut parts = vec![];

        let seg = match sign {
            true => "sign",
            false => "verify",
        };
        parts.push(seg);

        if let Some(req_path) = req_path {
            parts.push(
                req_path
                    .as_str()
                    .trim_start_matches('/')
                    .trim_end_matches('/'),
            );
        }
        let object_id = object_id.to_string();
        parts.push(&object_id);

        let p = parts.join("/");
        self.service_url.join(&p).unwrap()
    }

    fn encode_common_headers(&self, com_req: &CryptoOutputRequestCommon, http_req: &mut Request) {
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

    fn encode_verify_object_request(&self, req: &CryptoVerifyObjectOutputRequest) -> Request {
        let url = self.format_url(false, req.common.req_path.as_ref(), &req.object.object_id);

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req.insert_header(cyfs_base::CYFS_OBJECT_ID, req.object.object_id.to_string());
        http_req.insert_header(cyfs_base::CYFS_SIGN_TYPE, req.sign_type.to_string());

        let verify_type = req.sign_object.as_str();
        match &req.sign_object {
            VerifyObjectType::Owner | VerifyObjectType::Own => {}
            VerifyObjectType::Object(sign_object) => {
                http_req.insert_header(
                    cyfs_base::CYFS_SIGN_OBJ_ID,
                    sign_object.object_id.to_string(),
                );
                if let Some(obj) = &sign_object.object_raw {
                    http_req.insert_header(cyfs_base::CYFS_SIGN_OBJ, hex::encode(obj));
                }
            }
            VerifyObjectType::Sign(signs) => {
                http_req.insert_header(cyfs_base::CYFS_VERIFY_SIGNS, signs.encode_string());
            }
        }

        http_req.insert_header(cyfs_base::CYFS_VERIFY_TYPE, verify_type);

        http_req
    }

    async fn decode_verify_object_response(
        object_id: &ObjectId,
        mut resp: Response,
    ) -> BuckyResult<CryptoVerifyObjectOutputResponse> {
        let body = resp.body_string().await.map_err(|e| {
            let msg = format!(
                "read verify object response from crypto failed, read body string error! obj={}, {}",
                object_id, e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!("verify resp body: {}", body);

        let resp = CryptoVerifyObjectResponse::decode_string(&body)?;

        info!("verify object response: obj={}, resp={:?}", object_id, resp);

        Ok(resp)
    }

    // 校验一个对象是否有指定object的签名
    pub async fn verify_object(
        &self,
        req: CryptoVerifyObjectRequest,
    ) -> BuckyResult<CryptoVerifyObjectResponse> {
        let mut http_req = self.encode_verify_object_request(&req);
        http_req.set_body(req.object.object_raw);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = Self::decode_verify_object_response(&req.object.object_id, resp).await?;
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            Err(e)
        }
    }

    fn encode_sign_object_request(&self, req: &CryptoSignObjectRequest) -> Request {
        let url = self.format_url(true, req.common.req_path.as_ref(), &req.object.object_id);

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(&req.common, &mut http_req);
        http_req.insert_header(cyfs_base::CYFS_OBJECT_ID, req.object.object_id.to_string());
        http_req.insert_header(cyfs_base::CYFS_SIGN_FLAGS, req.flags.to_string());

        http_req
    }

    async fn decode_sign_object_response(
        object_id: &ObjectId,
        mut resp: Response,
    ) -> BuckyResult<CryptoSignObjectOutputResponse> {
        let sign_result: SignObjectResult =
            RequestorHelper::decode_header(&resp, cyfs_base::CYFS_SIGN_RET)?;

        info!(
            "sign object from crypto success: obj={}, ret={}",
            object_id,
            sign_result.to_string()
        );

        let ret = match sign_result {
            SignObjectResult::Signed => {
                let buf = resp.body_bytes().await.map_err(|e| {
                    let msg = format!(
                        "get object from sign object resp failed, read body bytes error! obj={} {}",
                        object_id, e
                    );
                    error!("{}", msg);

                    BuckyError::from(msg)
                })?;

                let object = NONObjectInfo::new_from_object_raw(buf)?;

                CryptoSignObjectResponse {
                    result: sign_result,
                    object: Some(object),
                }
            }
            SignObjectResult::Pending => CryptoSignObjectResponse {
                result: sign_result,
                object: None,
            },
        };

        Ok(ret)
    }

    pub async fn sign_object(
        &self,
        req: CryptoSignObjectRequest,
    ) -> BuckyResult<CryptoSignObjectResponse> {
        let mut http_req = self.encode_sign_object_request(&req);
        http_req.set_body(req.object.object_raw);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = Self::decode_sign_object_response(&req.object.object_id, resp).await?;
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl CryptoOutputProcessor for CryptoRequestor {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectOutputRequest,
    ) -> BuckyResult<CryptoVerifyObjectOutputResponse> {
        Self::verify_object(&self, req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectOutputRequest,
    ) -> BuckyResult<CryptoSignObjectOutputResponse> {
        Self::sign_object(&self, req).await
    }
}
