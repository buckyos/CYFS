use super::request::*;
use crate::base::*;
use cyfs_base::*;

use http_types::{Method, Request, Response, StatusCode, Url};
use std::sync::Arc;

pub struct CryptoRequestor {
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl Default for CryptoRequestor {
    fn default() -> Self {
        let service_addr = format!("127.0.0.1:{}", cyfs_base::NON_STACK_HTTP_PORT);

        Self::new_tcp(&service_addr)
    }
}

impl CryptoRequestor {
    pub fn new_tcp(service_addr: &str) -> Self {
        let tcp_requestor = TcpHttpRequestor::new(service_addr);
        Self::new(Arc::new(Box::new(tcp_requestor)))
    }

    pub fn new(requestor: HttpRequestorRef) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/crypto/", addr);
        let url = Url::parse(&url).unwrap();

        Self {
            requestor,
            service_url: url,
        }
    }

    // url支持下面的格式，其中device_id是可选
    // {host:port}/crypto/verify/[device_id]/object_id
    fn format_url(&self, sign: bool, target: &Option<DeviceId>, object_id: &ObjectId) -> Url {
        let seg = match sign {
            true => "sign",
            false => "verify",
        };

        if target.is_some() {
            self.service_url
                .join(&format!(
                    "{}/{}/{}",
                    seg,
                    target.as_ref().unwrap().to_string(),
                    object_id.to_string()
                ))
                .unwrap()
        } else {
            self.service_url
                .join(&format!("{}/{}", seg, object_id.to_string()))
                .unwrap()
        }
    }

    fn encode_verify_by_object_request(&self, req: &CryptoVerifyByObjectRequest) -> Request {
        let url = self.format_url(false, &req.target, &req.object_id);

        let mut http_req = Request::new(Method::Post, url);
        http_req.insert_header(
            cyfs_base::CYFS_VERIFY_TYPE,
            VerifyObjectType::Object.to_string(),
        );
        http_req.insert_header(cyfs_base::CYFS_SIGN_TYPE, req.sign_type.to_string());

        if let Some(obj_id) = &req.sign_object_id {
            http_req.insert_header(cyfs_base::CYFS_SIGN_OBJ_ID, obj_id.to_string());
        }

        if let Some(obj) = &req.sign_object {
            http_req.insert_header(cyfs_base::CYFS_SIGN_OBJ, hex::encode(obj));
        }

        http_req
    }

    fn encode_verify_by_owner_request(&self, req: &CryptoVerifyByOwnerRequest) -> Request {
        let url = self.format_url(false, &req.target, &req.object_id);

        let mut http_req = Request::new(Method::Post, url);
        http_req.insert_header(
            cyfs_base::CYFS_VERIFY_TYPE,
            VerifyObjectType::Owner.to_string(),
        );
        http_req.insert_header(cyfs_base::CYFS_SIGN_TYPE, req.sign_type.to_string());

        http_req
    }

    async fn request(&self, object_id: &ObjectId, http_req: Request) -> BuckyResult<Response> {
        let mut resp = self.requestor.request(http_req).await?;

        match resp.status() {
            StatusCode::Ok => Ok(resp),
            code @ _ => {
                let msg = resp.body_string().await.unwrap_or("".to_owned());
                let msg = format!(
                    "verify/sign object from crypto failed: obj={} status={} msg={}",
                    object_id, code, msg
                );
                error!("{}", msg);

                let err_code = RequestorHelper::trans_status_code(code);

                Err(BuckyError::new(err_code, msg))
            }
        }
    }

    async fn verify_request(
        &self,
        object_id: &ObjectId,
        http_req: Request,
    ) -> BuckyResult<CryptoVerifyObjectResponse> {
        let mut resp = self.request(object_id, http_req).await?;

        info!("verify object from crypto success: obj={}", object_id);

        let body = resp.body_string().await.map_err(|e| {
            let msg = format!(
                "get verify result from crypto failed, read body string error! obj={} {}",
                object_id, e
            );
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        info!("verify resp body: {}", body);

        let result = VerifyObjectResult::decode_string(&body)?;

        let resp = CryptoVerifyObjectResponse { result };

        Ok(resp)
    }

    // 校验一个对象是否有指定object的签名
    pub async fn verify_by_object(
        &self,
        req: CryptoVerifyByObjectRequest,
    ) -> BuckyResult<CryptoVerifyObjectResponse> {
        let mut http_req = self.encode_verify_by_object_request(&req);
        http_req.set_body(req.object_raw);

        self.verify_request(&req.object_id, http_req).await
    }

    // 校验一个对象是否有owner的签名
    pub async fn verify_by_owner(
        &self,
        req: CryptoVerifyByOwnerRequest,
    ) -> BuckyResult<CryptoVerifyObjectResponse> {
        let mut http_req = self.encode_verify_by_owner_request(&req);
        http_req.set_body(req.object_raw);

        self.verify_request(&req.object_id, http_req).await
    }

    pub async fn verify_by_sign(
        &self,
        _req: CryptoVerifyBySignRequest,
    ) -> BuckyResult<CryptoVerifyObjectResponse> {
        unimplemented!();
    }

    fn encode_sign_object_request(&self, req: &CryptoSignObjectRequest) -> Request {
        let url = self.format_url(true, &req.target, &req.object_id);

        let mut http_req = Request::new(Method::Post, url);
        http_req.insert_header(cyfs_base::CYFS_FLAGS, req.flags.to_string());

        if let Some(dec_id) = &req.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        }

        http_req
    }

    pub async fn sign_object(
        &self,
        req: CryptoSignObjectRequest,
    ) -> BuckyResult<CryptoSignObjectResponse> {
        let mut http_req = self.encode_sign_object_request(&req);
        http_req.set_body(req.object_raw);

        let mut resp = self.request(&req.object_id, http_req).await?;

        let sign_result: SignObjectResult =
            RequestorHelper::decode_header(&resp, cyfs_base::CYFS_SIGN_RET)?;

        info!(
            "sign object from crypto success: obj={}, ret={}",
            req.object_id,
            sign_result.to_string()
        );

        let ret = match sign_result {
            SignObjectResult::Signed => {
                let object_id = &req.object_id;
                let buf = resp.body_bytes().await.map_err(|e| {
                    let msg = format!(
                        "get object from sign object resp failed, read body bytes error! obj={} {}",
                        object_id, e
                    );
                    error!("{}", msg);

                    BuckyError::from(msg)
                })?;

                let (object, _) = AnyNamedObject::raw_decode(&buf).map_err(|e| {
                    error!(
                        "decode object from sign object resp bytes error: obj={} {}",
                        object_id, e,
                    );
                    e
                })?;

                CryptoSignObjectResponse {
                    result: sign_result,
                    object: Some(SignedObject {
                        object_raw: buf,
                        object,
                    }),
                }
            }
            SignObjectResult::Pending => CryptoSignObjectResponse {
                result: sign_result,
                object: None,
            },
        };

        Ok(ret)
    }
}
