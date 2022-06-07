use crate::crypto::*;
use crate::{non::NONInputHttpRequest, non_api::NONRequestUrlParser};
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
        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取dec字段
        let dec_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_DEC_ID)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = CryptoInputRequestCommon {
            req_path: None,

            source: req.source.clone(),
            protocol: req.protocol.clone(),

            dec_id,
            target,
            flags: flags.unwrap_or(0),
        };

        Ok(ret)
    }

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
        let param = NONRequestUrlParser::parse_put_param(&req.request)?;
        let mut common = Self::decode_common_headers(&req)?;
        common.req_path = param.req_path;

        let mut object = NONObjectInfo::new(param.object_id, body, None);
        object.decode_and_verify()?;

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

                let mut sign_object = NONSlimObjectInfo::new(object_id, object_raw,None);
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
        let param = NONRequestUrlParser::parse_put_param(&req.request)?;
        let mut common = Self::decode_common_headers(&req)?;
        common.req_path = param.req_path;

        let mut object = NONObjectInfo::new(param.object_id, body, None);
        object.decode_and_verify()?;

        let flags = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_SIGN_FLAGS)?;

        let req = CryptoSignObjectInputRequest {
            common,
            object,
            flags,
        };

        info!("recv sign object request: {:?}", req);

        self.processor.sign_object(req).await
    }
}
