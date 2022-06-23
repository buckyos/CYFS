use super::url::*;
use crate::front::FrontRequestObjectFormat;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use http_types::{Request, StatusCode};
use tide::Response;

#[derive(Clone)]
pub(crate) struct NONRequestHandler {
    processor: NONInputProcessorRef,
}

impl NONRequestHandler {
    pub fn new(processor: NONInputProcessorRef) -> Self {
        Self { processor }
    }

    // 提取action字段
    fn decode_action<State>(
        req: &NONInputHttpRequest<State>,
        default_action: NONAction,
    ) -> BuckyResult<NONAction> {
        match RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_NON_ACTION)? {
            Some(v) => Ok(v),
            None => Ok(default_action),
        }
    }

    // 解析通用header字段
    fn decode_common_headers<State>(
        req: &NONInputHttpRequest<State>,
    ) -> BuckyResult<NONInputRequestCommon> {
        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取dec字段
        let dec_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_DEC_ID)?;

        // 尝试提取default_action字段
        let level =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_API_LEVEL)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = NONInputRequestCommon {
            req_path: None,

            source: req.source.clone(),
            protocol: req.protocol.clone(),

            dec_id,
            level: level.unwrap_or_default(),
            target,
            flags: flags.unwrap_or(0),
        };

        Ok(ret)
    }

    pub fn encode_put_object_response(resp: NONPutObjectInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        RequestorHelper::encode_header(&mut http_resp, cyfs_base::CYFS_RESULT, &resp.result);
        RequestorHelper::encode_opt_header(
            &mut http_resp,
            cyfs_base::CYFS_OBJECT_EXPIRES_TIME,
            &resp.object_expires_time,
        );
        RequestorHelper::encode_opt_header(
            &mut http_resp,
            cyfs_base::CYFS_OBJECT_UPDATE_TIME,
            &resp.object_update_time,
        );

        // 设置标准的http header
        if let Some(object_update_time) = resp.object_update_time {
            RequestorHelper::encode_time_header(
                &mut http_resp,
                http_types::headers::LAST_MODIFIED,
                object_update_time,
            );
        }
        if let Some(object_expires_time) = resp.object_expires_time {
            RequestorHelper::encode_time_header(
                &mut http_resp,
                http_types::headers::EXPIRES,
                object_expires_time,
            );
        }

        http_resp.into()
    }

    pub async fn process_put_object_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_put_object(req).await;
        match ret {
            Ok(resp) => Self::encode_put_object_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_put_object<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, NONAction::PutObject)?;
        if action != NONAction::PutObject {
            let msg = format!("invalid non put_object action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let param = NONRequestUrlParser::parse_put_param(&req.request)?;
        let mut common = Self::decode_common_headers(&req)?;

        let object = NONRequestorHelper::decode_object_info(&mut req.request).await?;

        common.req_path = param.req_path;
        // common.request = Some(req.request.into());

        let put_req = NONPutObjectInputRequest { common, object };

        info!("recv put_object request: {}", put_req);

        self.processor.put_object(put_req).await
    }

    pub fn encode_get_object_response_times(
        http_resp: &mut http_types::Response,
        resp: &NONGetObjectInputResponse,
    ) {
        RequestorHelper::encode_opt_header(
            http_resp,
            cyfs_base::CYFS_OBJECT_EXPIRES_TIME,
            &resp.object_expires_time,
        );
        RequestorHelper::encode_opt_header(
            http_resp,
            cyfs_base::CYFS_OBJECT_UPDATE_TIME,
            &resp.object_update_time,
        );

        // 设置标准的http header
        if let Some(object_update_time) = resp.object_update_time {
            RequestorHelper::encode_time_header(
                http_resp,
                http_types::headers::LAST_MODIFIED,
                object_update_time,
            );
        }
        if let Some(object_expires_time) = resp.object_expires_time {
            RequestorHelper::encode_time_header(
                http_resp,
                http_types::headers::EXPIRES,
                object_expires_time,
            );
        }
    }

    pub fn encode_get_object_response(
        resp: NONGetObjectInputResponse,
        format: FrontRequestObjectFormat,
    ) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        Self::encode_get_object_response_times(&mut http_resp, &resp);

        match format {
            FrontRequestObjectFormat::Raw | FrontRequestObjectFormat::Default => {
                NONRequestorHelper::encode_object_info(&mut http_resp, resp.object);
            }
            FrontRequestObjectFormat::Json => {
                http_resp
                    .insert_header(cyfs_base::CYFS_OBJECT_ID, resp.object.object_id.to_string());

                http_resp.set_body(resp.object.object().format_json().to_string());
                http_resp.set_content_type(::tide::http::mime::JSON);
            }
        }

        if let Some(attr) = &resp.attr {
            http_resp.insert_header(cyfs_base::CYFS_ATTRIBUTES, attr.flags().to_string());
        }

        http_resp.into()
    }

    pub async fn process_get_request<State>(&self, req: NONInputHttpRequest<State>) -> Response {
        // get操作存在get_object和select_object两种请求，需要通过action来进一步区分
        let ret = Self::decode_action(&req, NONAction::GetObject);

        match ret {
            Ok(NONAction::GetObject) => {
                let ret = self.on_get_object(req).await;
                match ret {
                    Ok(resp) => {
                        Self::encode_get_object_response(resp, FrontRequestObjectFormat::Raw)
                    }
                    Err(e) => RequestorHelper::trans_error(e),
                }
            }
            Ok(NONAction::SelectObject) => {
                let ret = self.on_select_object(req).await;
                match ret {
                    Ok(resp) => Self::encode_select_object_response(resp),
                    Err(e) => RequestorHelper::trans_error(e),
                }
            }
            Ok(action) => {
                let msg = format!("invalid non get action! {:?}", action);
                error!("{}", msg);

                let e = BuckyError::new(BuckyErrorCode::InvalidData, msg);
                RequestorHelper::trans_error(e)
            }
            Err(e) => {
                let msg = format!("decode non get action error! {}", e);
                error!("{}", msg);

                let e = BuckyError::new(BuckyErrorCode::InvalidData, msg);
                RequestorHelper::trans_error(e)
            }
        }
    }

    async fn on_get_object<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let param = NONRequestUrlParser::parse_get_param(&req.request)?;
        let mut common = Self::decode_common_headers(&req)?;

        // 优先尝试从header里面提取
        let inner_path = match RequestorHelper::decode_optional_header(
            &req.request,
            cyfs_base::CYFS_INNER_PATH,
        )? {
            Some(v) => Some(v),
            None => param.inner_path,
        };

        common.req_path = param.req_path;
        //common.request = Some(req.request.into());

        let get_req = NONGetObjectInputRequest {
            common,
            object_id: param.object_id,

            inner_path,
        };

        info!("recv get_object request: {}", get_req);

        self.processor.get_object(get_req).await
    }

    pub async fn process_post_object_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_post_object(req).await;
        match ret {
            Ok(resp) => Self::encode_post_object_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub fn encode_post_object_response(resp: NONPostObjectInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        if let Some(object) = resp.object {
            NONRequestorHelper::encode_object_info(&mut http_resp, object);
        }

        http_resp.into()
    }

    async fn on_post_object<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        let action = Self::decode_action(&req, NONAction::PostObject)?;
        if action != NONAction::PostObject {
            let msg = format!("invalid non post_object action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let param = NONRequestUrlParser::parse_put_param(&req.request)?;
        let mut common = Self::decode_common_headers(&req)?;

        let object = NONRequestorHelper::decode_object_info(&mut req.request).await?;

        common.req_path = param.req_path;
        // common.request = Some(req.request.into());

        let post_req = NONPostObjectInputRequest { common, object };

        info!("recv post_object request: {}", post_req);

        self.processor.post_object(post_req).await
    }

    pub fn encode_select_object_response(resp: NONSelectObjectInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        SelectResponse::encode_objects(&mut http_resp, &resp.objects);

        http_resp.into()
    }

    async fn on_select_object<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        let param = NONRequestUrlParser::parse_select_param(&req.request)?;
        let mut common = Self::decode_common_headers(&req)?;

        let filter = SelectFilterUrlCodec::decode(req.request.url())?;

        let http_req: Request = req.request.into();
        let opt = SelectOptionCodec::decode(&http_req)?;

        common.req_path = param.req_path;
        // common.request = Some(http_req);

        let select_req = NONSelectObjectInputRequest {
            common,
            filter,
            opt,
        };

        info!("recv select_object request: {}", select_req);

        self.processor.select_object(select_req).await
    }

    pub async fn process_delete_object_request<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_delete_object(req).await;
        match ret {
            Ok(resp) => Self::encode_delete_object_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub fn encode_delete_object_response(resp: NONDeleteObjectInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        if let Some(info) = resp.object {
            NONRequestorHelper::encode_object_info(&mut http_resp, info);
        }

        http_resp.into()
    }

    async fn on_delete_object<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, NONAction::DeleteObject)?;
        if action != NONAction::DeleteObject {
            let msg = format!("invalid non delete_object action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let param = NONRequestUrlParser::parse_get_param(&req.request)?;
        let mut common = Self::decode_common_headers(&req)?;

        common.req_path = param.req_path;
        // common.request = Some(req.request.into());

        let delete_req = NONDeleteObjectInputRequest {
            common,
            object_id: param.object_id,
            inner_path: param.inner_path,
        };

        info!("recv delete_object request: {}", delete_req);

        self.processor.delete_object(delete_req).await
    }
}
