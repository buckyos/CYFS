use crate::non_api::RequestHandlerHelper;
use cyfs_base::*;
use cyfs_lib::*;

struct NDNRequestUtil;

// FIXME 选择哪些字段可以替换
impl NDNRequestUtil {
    fn update_request_common(origin: &mut NDNInputRequestCommon, handler: NDNInputRequestCommon) {
        origin.req_path = handler.req_path;
        origin.target = handler.target;
        origin.referer_object = handler.referer_object;
        origin.flags = handler.flags;
    }

    fn update_put_data_request(
        origin: &mut NDNPutDataInputRequest,
        handler: NDNPutDataInputRequest,
    ) {
        Self::update_request_common(&mut origin.common, handler.common);
    }
    fn update_put_data_response(
        origin: &mut NDNPutDataInputResponse,
        handler: NDNPutDataInputResponse,
    ) {
        origin.result = handler.result;
    }

    fn update_get_data_request(
        origin: &mut NDNGetDataInputRequest,
        handler: NDNGetDataInputRequest,
    ) {
        origin.object_id = handler.object_id;
        origin.inner_path = handler.inner_path;
        Self::update_request_common(&mut origin.common, handler.common);
    }
    fn update_get_data_response(
        origin: &mut NDNGetDataInputResponse,
        handler: NDNGetDataInputResponse,
    ) {
        origin.attr = handler.attr;
    }

    fn update_delete_data_request(
        origin: &mut NDNDeleteDataInputRequest,
        handler: NDNDeleteDataInputRequest,
    ) {
        origin.object_id = handler.object_id;
        origin.inner_path = handler.inner_path;

        Self::update_request_common(&mut origin.common, handler.common);
    }

    fn update_delete_data_response(
        _origin: &mut NDNDeleteDataInputResponse,
        _handler: NDNDeleteDataInputResponse,
    ) {
        // origin.object_id = handler.object_id;
    }
}

// put_data
impl RequestHandlerHelper<NDNPutDataInputRequest> for NDNPutDataInputRequest {
    fn update(&mut self, handler: NDNPutDataInputRequest) {
        NDNRequestUtil::update_put_data_request(self, handler)
    }

    fn debug_info(&self) -> String {
        self.object_id.to_string()
    }

    fn req_path(&self) -> &Option<String> {
        &self.common.req_path
    }

    fn source(&self) -> &RequestSourceInfo {
        &self.common.source
    }
}
impl RequestHandlerHelper<NDNPutDataInputResponse> for NDNPutDataInputResponse {
    fn update(&mut self, handler: NDNPutDataInputResponse) {
        NDNRequestUtil::update_put_data_response(self, handler)
    }
}

// get_data
impl RequestHandlerHelper<NDNGetDataInputRequest> for NDNGetDataInputRequest {
    fn update(&mut self, handler: Self) {
        NDNRequestUtil::update_get_data_request(self, handler)
    }

    fn debug_info(&self) -> String {
        self.object_id.to_string()
    }

    fn req_path(&self) -> &Option<String> {
        &self.common.req_path
    }

    fn source(&self) -> &RequestSourceInfo {
        &self.common.source
    }
}
impl RequestHandlerHelper<NDNGetDataInputResponse> for NDNGetDataInputResponse {
    fn update(&mut self, handler: Self) {
        NDNRequestUtil::update_get_data_response(self, handler)
    }
}

// delete_data
impl RequestHandlerHelper<NDNDeleteDataInputRequest> for NDNDeleteDataInputRequest {
    fn update(&mut self, handler: Self) {
        NDNRequestUtil::update_delete_data_request(self, handler)
    }

    fn debug_info(&self) -> String {
        self.object_id.to_string()
    }

    fn req_path(&self) -> &Option<String> {
        &self.common.req_path
    }

    fn source(&self) -> &RequestSourceInfo {
        &self.common.source
    }
}

impl RequestHandlerHelper<NDNDeleteDataInputResponse> for NDNDeleteDataInputResponse {
    fn update(&mut self, handler: Self) {
        NDNRequestUtil::update_delete_data_response(self, handler)
    }
}

// interest handler
impl RequestHandlerHelper<InterestHandlerRequest> for InterestHandlerRequest {
    fn update(&mut self, handler: Self) {
        self.referer = handler.referer;
        self.prefer_type = handler.prefer_type;
    }

    fn debug_info(&self) -> String {
        self.session_id.value().to_string()
    }

    fn req_path(&self) -> &Option<String> {
        &None
    }

    fn source(&self) -> &RequestSourceInfo {
        unimplemented!();
    }
}

impl RequestHandlerHelper<InterestHandlerResponse> for InterestHandlerResponse {
    fn update(&mut self, handler: Self) {
        *self = handler.clone();
    }
}
