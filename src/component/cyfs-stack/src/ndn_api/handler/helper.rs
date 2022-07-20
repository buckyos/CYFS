use cyfs_lib::*;
use crate::non_api::RequestHandlerHelper;

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
}
impl RequestHandlerHelper<NDNPutDataInputResponse> for NDNPutDataInputResponse {
    fn update(&mut self, handler: NDNPutDataInputResponse) {
        NDNRequestUtil::update_put_data_response(self, handler)
    }

    fn debug_info(&self) -> String {
        unimplemented!();
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
}
impl RequestHandlerHelper<NDNGetDataInputResponse> for NDNGetDataInputResponse {
    fn update(&mut self, handler: Self) {
        NDNRequestUtil::update_get_data_response(self, handler)
    }

    fn debug_info(&self) -> String {
        unimplemented!();
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
}

impl RequestHandlerHelper<NDNDeleteDataInputResponse> for NDNDeleteDataInputResponse {
    fn update(&mut self, handler: Self) {
        NDNRequestUtil::update_delete_data_response(self, handler)
    }

    fn debug_info(&self) -> String {
        unimplemented!();
    }
}


// interest handler
impl RequestHandlerHelper<InterestHandlerRequest> for InterestHandlerRequest {
    fn update(&mut self, handler: Self) {
        self.interest = handler.interest.clone();
    }

    fn debug_info(&self) -> String {
        self.interest.session_id.value().to_string()
    }
}

impl RequestHandlerHelper<InterestHandlerResponse> for InterestHandlerResponse {
    fn update(&mut self, handler: Self) {
        *self = handler.clone(); 
    }

    fn debug_info(&self) -> String {
        unimplemented!()
    }
}