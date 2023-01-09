use crate::crypto::*;
use crate::non::NONInputHttpRequest;
use cyfs_base::*;
use cyfs_lib::*;

use tide::{Response, StatusCode};

#[derive(Clone)]
pub(crate) struct CryptoRequestHandler {
    processor: CryptoInputProcessorRef,
}

impl CryptoRequestHandler {
    pub fn new(processor: CryptoInputProcessorRef) -> Self {
        Self { processor }
    }

    fn decode_common_headers<State>(
        req: &NONInputHttpRequest<State>,
    ) -> BuckyResult<CryptoInputRequestCommon> {
        // req_path
        let req_path = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_REQ_PATH,
        )?;

        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = CryptoInputRequestCommon {
            req_path,
            source: req.source.clone(),
            target,
            flags: flags.unwrap_or(0),
        };

        Ok(ret)
    }

    // verify_object
    pub async fn process_verify_object<State>(
        &self,
        req: NONInputHttpRequest<State>,
        body: Vec<u8>,
    ) -> Response {
        let ret = self.on_verify_request(req, body).await;
        match ret {
            Ok(resp) => Self::encode_verify_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    fn encode_verify_response(resp: CryptoVerifyObjectInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        let body = resp.encode_string();

        debug!("verify object resp: {:?}", body);
        http_resp.set_content_type(::tide::http::mime::JSON);
        http_resp.set_body(body);

        http_resp.into()
    }

    async fn on_verify_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
        body: Vec<u8>,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        let object = NONObjectInfo::new_from_object_raw(body)?;

        let sign_type: VerifySignType =
            RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_SIGN_TYPE)?;

        let verify_type: String =
            RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_VERIFY_TYPE)?;
        let sign_object = match verify_type.as_str() {
            "owner" => VerifyObjectType::Owner,
            "object" => {
                let object_id: ObjectId =
                    RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_SIGN_OBJ_ID)?;

                let object_raw = RequestorHelper::decode_optional_hex_header(
                    &req.request,
                    cyfs_base::CYFS_SIGN_OBJ,
                )?;

                let mut sign_object = NONSlimObjectInfo::new(object_id, object_raw, None);
                sign_object.decode_and_verify()?;

                VerifyObjectType::Object(sign_object)
            }
            "sign" => {
                let signs = RequestorHelper::decode_json_header(
                    &req.request,
                    cyfs_base::CYFS_VERIFY_SIGNS,
                )?;
                VerifyObjectType::Sign(signs)
            }
            _ => {
                let msg = format!("unknown cyfs-verify-type header value: {}", verify_type);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        };

        let req = CryptoVerifyObjectInputRequest {
            common,
            object,
            sign_type,
            sign_object,
        };

        info!("recv verify object request: {:?}", req);
        self.processor.verify_object(req).await
    }

    // sign_object
    pub async fn process_sign_object<State>(
        &self,
        req: NONInputHttpRequest<State>,
        body: Vec<u8>,
    ) -> Response {
        let ret = self.on_sign_request(req, body).await;
        match ret {
            Ok(resp) => Self::encode_sign_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    fn encode_sign_response(resp: CryptoSignObjectInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.insert_header(cyfs_base::CYFS_SIGN_RET, resp.result.to_string());
        if let Some(object) = resp.object {
            http_resp.set_body(object.object_raw);
        }

        http_resp.into()
    }

    async fn on_sign_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
        body: Vec<u8>,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        let object = NONObjectInfo::new_from_object_raw(body)?;

        let flags = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_CRYPTO_FLAGS)?;

        let req = CryptoSignObjectInputRequest {
            common,
            object,
            flags,
        };

        info!("recv sign object request: {:?}", req);

        self.processor.sign_object(req).await
    }

    // encrypt_data
    pub async fn process_encrypt_data<State>(
        &self,
        req: NONInputHttpRequest<State>,
        body: Vec<u8>,
    ) -> Response {
        let ret = self.on_encrypt_request(req, body).await;
        match ret {
            Ok(resp) => Self::encode_encrypt_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    fn encode_encrypt_response(resp: CryptoEncryptDataInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
        RequestorHelper::encode_opt_header(&mut http_resp, CYFS_AES_KEY, &resp.aes_key);
        http_resp.set_body(resp.result);

        http_resp.into()
    }

    async fn on_encrypt_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
        body: Vec<u8>,
    ) -> BuckyResult<CryptoEncryptDataInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        let data = if body.len() > 0 { Some(body) } else { None };

        let encrypt_type: CryptoEncryptType =
            RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_ENCRYPT_TYPE)?;

        let flags = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_CRYPTO_FLAGS)?;

        let req = CryptoEncryptDataInputRequest {
            common,
            encrypt_type,
            data,
            flags,
        };

        info!("recv encrypt data request: {:?}", req);
        self.processor.encrypt_data(req).await
    }

    // decrypt_data
    pub async fn process_decrypt_data<State>(
        &self,
        req: NONInputHttpRequest<State>,
        body: Vec<u8>,
    ) -> Response {
        let ret = self.on_decrypt_request(req, body).await;
        match ret {
            Ok(resp) => Self::encode_decrypt_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    fn encode_decrypt_response(resp: CryptoDecryptDataInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
        RequestorHelper::encode_header(&mut http_resp, CYFS_DECRYPT_RET, &resp.result);
        http_resp.set_body(resp.data);

        http_resp.into()
    }

    async fn on_decrypt_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
        body: Vec<u8>,
    ) -> BuckyResult<CryptoDecryptDataInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        let decrypt_type: CryptoDecryptType =
            RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_DECRYPT_TYPE)?;

        let flags = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_CRYPTO_FLAGS)?;

        let req = CryptoDecryptDataInputRequest {
            common,
            decrypt_type,
            data: body,
            flags,
        };

        info!("recv decrypt data request: {:?}", req);
        self.processor.decrypt_data(req).await
    }
}
