use crate::non_api::RequestHandlerHelper;
use cyfs_base::*;
use cyfs_lib::*;

struct CryptoRequestUtil;

// FIXME 选择哪些字段可以替换
impl CryptoRequestUtil {
    fn update_request_common(
        origin: &mut CryptoInputRequestCommon,
        handler: CryptoInputRequestCommon,
    ) {
        origin.req_path = handler.req_path;
        origin.target = handler.target;
        origin.flags = handler.flags;
    }

    fn update_sign_object_request(
        origin: &mut CryptoSignObjectInputRequest,
        handler: CryptoSignObjectInputRequest,
    ) {
        Self::update_request_common(&mut origin.common, handler.common);
        origin.object = handler.object;
        origin.flags = handler.flags;
    }
    fn update_sign_object_response(
        origin: &mut CryptoSignObjectInputResponse,
        handler: CryptoSignObjectInputResponse,
    ) {
        origin.result = handler.result;
        origin.object = handler.object;
    }

    fn update_verify_object_request(
        origin: &mut CryptoVerifyObjectInputRequest,
        handler: CryptoVerifyObjectInputRequest,
    ) {
        Self::update_request_common(&mut origin.common, handler.common);

        origin.object = handler.object;
        origin.sign_type = handler.sign_type;
        origin.sign_object = handler.sign_object;
    }
    fn update_verify_object_response(
        origin: &mut CryptoVerifyObjectInputResponse,
        handler: CryptoVerifyObjectInputResponse,
    ) {
        origin.result = handler.result;
    }
}

// sign_object
impl RequestHandlerHelper<CryptoSignObjectInputRequest> for CryptoSignObjectInputRequest {
    fn update(&mut self, handler: CryptoSignObjectInputRequest) {
        CryptoRequestUtil::update_sign_object_request(self, handler)
    }

    fn debug_info(&self) -> String {
        self.object.object_id.to_string()
    }

    fn req_path(&self) -> &Option<String> {
        &self.common.req_path
    }

    fn source(&self) -> &RequestSourceInfo {
        &self.common.source
    }
}
impl RequestHandlerHelper<CryptoSignObjectInputResponse> for CryptoSignObjectInputResponse {
    fn update(&mut self, handler: CryptoSignObjectInputResponse) {
        CryptoRequestUtil::update_sign_object_response(self, handler)
    }
}

// verify_object
impl RequestHandlerHelper<CryptoVerifyObjectInputRequest> for CryptoVerifyObjectInputRequest {
    fn update(&mut self, handler: Self) {
        CryptoRequestUtil::update_verify_object_request(self, handler)
    }

    fn debug_info(&self) -> String {
        self.object.object_id.to_string()
    }

    fn req_path(&self) -> &Option<String> {
        &self.common.req_path
    }

    fn source(&self) -> &RequestSourceInfo {
        &self.common.source
    }
}

impl RequestHandlerHelper<CryptoVerifyObjectInputResponse> for CryptoVerifyObjectInputResponse {
    fn update(&mut self, handler: Self) {
        CryptoRequestUtil::update_verify_object_response(self, handler)
    }
}
