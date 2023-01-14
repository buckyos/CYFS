use super::output_request::*;
use super::processor::*;
use super::request::*;
use crate::non::NONObjectInfo;
use crate::*;
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

    fn encode_common_headers(&self, com_req: &CryptoOutputRequestCommon, http_req: &mut Request) {
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

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());
    }

    fn encode_verify_object_request(&self, req: &CryptoVerifyObjectOutputRequest) -> Request {
        let url = self.service_url.join("verify").unwrap();

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
        let url = self.service_url.join("sign").unwrap();

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(&req.common, &mut http_req);
        http_req.insert_header(cyfs_base::CYFS_OBJECT_ID, req.object.object_id.to_string());
        http_req.insert_header(cyfs_base::CYFS_CRYPTO_FLAGS, req.flags.to_string());

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

    // encrypt
    fn encode_encrypt_data_request(&self, req: &CryptoEncryptDataOutputRequest) -> Request {
        let url = self.service_url.join("encrypt").unwrap();

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(&req.common, &mut http_req);
        http_req.insert_header(cyfs_base::CYFS_ENCRYPT_TYPE, req.encrypt_type.to_string());
        http_req.insert_header(cyfs_base::CYFS_CRYPTO_FLAGS, req.flags.to_string());

        http_req
    }

    async fn decode_encrypt_data_response(
        mut resp: Response,
    ) -> BuckyResult<CryptoEncryptDataOutputResponse> {
        let aes_key: Option<AesKey> =
            RequestorHelper::decode_optional_header(&resp, cyfs_base::CYFS_AES_KEY)?;

        let result = resp.body_bytes().await.map_err(|e| {
            let msg = format!(
                "get encrypt data from resp failed, read body bytes error! {}",
                e
            );
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        let resp = CryptoEncryptDataOutputResponse { aes_key, result };

        Ok(resp)
    }

    pub async fn encrypt_data(
        &self,
        req: CryptoEncryptDataOutputRequest,
    ) -> BuckyResult<CryptoEncryptDataOutputResponse> {
        let mut http_req = self.encode_encrypt_data_request(&req);
        let data_len = match &req.data {
            Some(data) => data.len(),
            None => 0,
        };

        if let Some(data) = req.data {
            http_req.set_body(data);
        }

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = Self::decode_encrypt_data_response(resp).await?;

            info!(
                "encrypt data success: data={}, type={}, ret={}",
                data_len,
                req.encrypt_type.to_string(),
                resp.result.len(),
            );

            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "encrypt data failed: data={}, type={}, {}",
                data_len,
                req.encrypt_type.to_string(),
                e,
            );
            Err(e)
        }
    }

    // decrypt
    fn encode_decrypt_data_request(&self, req: &CryptoDecryptDataOutputRequest) -> Request {
        let url = self.service_url.join("decrypt").unwrap();

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(&req.common, &mut http_req);
        http_req.insert_header(cyfs_base::CYFS_DECRYPT_TYPE, req.decrypt_type.to_string());
        http_req.insert_header(cyfs_base::CYFS_CRYPTO_FLAGS, req.flags.to_string());

        http_req
    }

    async fn decode_decrypt_data_response(
        mut resp: Response,
    ) -> BuckyResult<CryptoDecryptDataOutputResponse> {
        let result: DecryptDataResult =
            RequestorHelper::decode_header(&resp, cyfs_base::CYFS_DECRYPT_RET)?;

        let data = resp.body_bytes().await.map_err(|e| {
            let msg = format!(
                "get decrypt data from resp failed, read body bytes error! {}",
                e
            );
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        let resp = CryptoDecryptDataOutputResponse { result, data };

        Ok(resp)
    }

    pub async fn decrypt_data(
        &self,
        req: CryptoDecryptDataOutputRequest,
    ) -> BuckyResult<CryptoDecryptDataOutputResponse> {
        let mut http_req = self.encode_decrypt_data_request(&req);
        let data_len = req.data.len();
        http_req.set_body(req.data);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = Self::decode_decrypt_data_response(resp).await?;

            info!(
                "decrypt data crypto success: data={}, type={}, {}",
                data_len,
                req.decrypt_type.to_string(),
                resp,
            );

            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "decrypt data crypto failed: data={}, type={}, {}",
                data_len,
                req.decrypt_type.to_string(),
                e,
            );

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

    async fn encrypt_data(
        &self,
        req: CryptoEncryptDataOutputRequest,
    ) -> BuckyResult<CryptoEncryptDataOutputResponse> {
        Self::encrypt_data(&self, req).await
    }

    async fn decrypt_data(
        &self,
        req: CryptoDecryptDataOutputRequest,
    ) -> BuckyResult<CryptoDecryptDataOutputResponse> {
        Self::decrypt_data(&self, req).await
    }
}
