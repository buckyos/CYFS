use cyfs_lib::*;
use cyfs_base::*;


struct RequestUtil;

// FIXME 选择哪些字段可以替换
impl RequestUtil {
    fn update_request_common(origin: &mut NONInputRequestCommon, handler: NONInputRequestCommon) {
        origin.req_path = handler.req_path;
        origin.target = handler.target;
        origin.flags = handler.flags;
    }

    fn update_put_object_request(
        origin: &mut NONPutObjectInputRequest,
        handler: NONPutObjectInputRequest,
    ) {
        origin.object = handler.object;

        Self::update_request_common(&mut origin.common, handler.common);
    }
    fn update_put_object_response(
        origin: &mut NONPutObjectInputResponse,
        handler: NONPutObjectInputResponse,
    ) {
        origin.result = handler.result;
        origin.object_expires_time = handler.object_expires_time;
        origin.object_update_time = handler.object_update_time;
    }

    fn update_get_object_request(
        origin: &mut NONGetObjectInputRequest,
        handler: NONGetObjectInputRequest,
    ) {
        origin.object_id = handler.object_id;
        origin.inner_path = handler.inner_path;
        Self::update_request_common(&mut origin.common, handler.common);
    }
    fn update_get_object_response(
        origin: &mut NONGetObjectInputResponse,
        handler: NONGetObjectInputResponse,
    ) {
        origin.object = handler.object;
    }

    fn update_post_object_request(
        origin: &mut NONPostObjectInputRequest,
        handler: NONPostObjectInputRequest,
    ) {
        origin.object = handler.object;

        Self::update_request_common(&mut origin.common, handler.common);
    }
    fn update_post_object_response(
        origin: &mut NONPostObjectInputResponse,
        handler: NONPostObjectInputResponse,
    ) {
        origin.object = handler.object;
    }

    fn update_select_object_request(
        origin: &mut NONSelectObjectInputRequest,
        handler: NONSelectObjectInputRequest,
    ) {
        origin.filter = handler.filter;
        origin.opt = handler.opt;

        Self::update_request_common(&mut origin.common, handler.common);
    }

    fn update_select_object_response(
        origin: &mut NONSelectObjectInputResponse,
        handler: NONSelectObjectInputResponse,
    ) {
        origin.objects = handler.objects;
    }

    fn update_delete_object_request(
        origin: &mut NONDeleteObjectInputRequest,
        handler: NONDeleteObjectInputRequest,
    ) {
        origin.object_id = handler.object_id;
        Self::update_request_common(&mut origin.common, handler.common);
    }

    fn update_delete_object_response(
        origin: &mut NONDeleteObjectInputResponse,
        handler: NONDeleteObjectInputResponse,
    ) {
        origin.object = handler.object;
    }
}

pub(crate) trait RequestHandlerHelper<REQ> {
    fn update(&mut self, handler: REQ);
    fn debug_info(&self) -> String {
        unimplemented!();
    }
    fn req_path(&self) -> &Option<String> {
        unimplemented!();
    }
    fn source(&self) -> &RequestSourceInfo {
        unimplemented!();
    }
}

// put_object
impl RequestHandlerHelper<NONPutObjectInputRequest> for NONPutObjectInputRequest {
    fn update(&mut self, handler: NONPutObjectInputRequest) {
        RequestUtil::update_put_object_request(self, handler)
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
impl RequestHandlerHelper<NONPutObjectInputResponse> for NONPutObjectInputResponse {
    fn update(&mut self, handler: NONPutObjectInputResponse) {
        RequestUtil::update_put_object_response(self, handler)
    }
}

// get_object
impl RequestHandlerHelper<NONGetObjectInputRequest> for NONGetObjectInputRequest {
    fn update(&mut self, handler: Self) {
        RequestUtil::update_get_object_request(self, handler)
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
impl RequestHandlerHelper<NONGetObjectInputResponse> for NONGetObjectInputResponse {
    fn update(&mut self, handler: Self) {
        RequestUtil::update_get_object_response(self, handler)
    }
}

// post_object
impl RequestHandlerHelper<NONPostObjectInputRequest> for NONPostObjectInputRequest {
    fn update(&mut self, handler: Self) {
        RequestUtil::update_post_object_request(self, handler)
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

impl RequestHandlerHelper<NONPostObjectInputResponse> for NONPostObjectInputResponse {
    fn update(&mut self, handler: Self) {
        RequestUtil::update_post_object_response(self, handler)
    }
}

// select_object
impl RequestHandlerHelper<NONSelectObjectInputRequest> for NONSelectObjectInputRequest {
    fn update(&mut self, handler: Self) {
        RequestUtil::update_select_object_request(self, handler)
    }

    fn debug_info(&self) -> String {
        format!("{}", self.filter)
    }

    fn req_path(&self) -> &Option<String> {
        &self.common.req_path
    }

    fn source(&self) -> &RequestSourceInfo {
        &self.common.source
    }
}
impl RequestHandlerHelper<NONSelectObjectInputResponse> for NONSelectObjectInputResponse {
    fn update(&mut self, handler: Self) {
        RequestUtil::update_select_object_response(self, handler)
    }
}

// delete_object
impl RequestHandlerHelper<NONDeleteObjectInputRequest> for NONDeleteObjectInputRequest {
    fn update(&mut self, handler: Self) {
        RequestUtil::update_delete_object_request(self, handler)
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

impl RequestHandlerHelper<NONDeleteObjectInputResponse> for NONDeleteObjectInputResponse {
    fn update(&mut self, handler: Self) {
        RequestUtil::update_delete_object_response(self, handler)
    }
}

// 对BuckyResult<Response>的update处理
impl<T> RequestHandlerHelper<BuckyResult<T>> for BuckyResult<T>
where
    T: RequestHandlerHelper<T>,
{
    fn update(&mut self, handler: Self) {
        match self {
            Ok(v) => {
                match handler {
                    Ok(new) => v.update(new),
                    Err(e) => *self = Err(e),
                }
            }
            Err(_) => {
                *self = handler;
            }
        }
    }
}
