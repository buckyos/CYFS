use crate::ndn::*;
use crate::non::NONInputHttpRequest;
use cyfs_base::*;
use cyfs_lib::*;

use async_std::io::BufReader;
use http_types::StatusCode;
use tide::Response;

// 目前ndn使用non同样的http request
pub(crate) type NDNInputHttpRequest<State> = NONInputHttpRequest<State>;

// 从url params里面解析出所需要的参数
struct NDNGetDataUrlParams {
    common: NDNInputRequestCommon,
    object_id: ObjectId,
    inner_path: Option<String>,
    action: Option<NDNAction>,
    group: Option<String>,
}

#[derive(Clone)]
pub(crate) struct NDNRequestHandler {
    processor: NDNInputProcessorRef,
}

impl NDNRequestHandler {
    pub fn new(processor: NDNInputProcessorRef) -> Self {
        Self { processor }
    }

    // 提取action字段
    fn decode_action<State>(
        req: &NDNInputHttpRequest<State>,
        default_action: NDNAction,
    ) -> BuckyResult<NDNAction> {
        match Self::decode_option_action(req)? {
            Some(v) => Ok(v),
            None => Ok(default_action),
        }
    }

    fn decode_option_action<State>(
        req: &NDNInputHttpRequest<State>,
    ) -> BuckyResult<Option<NDNAction>> {
        RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_NDN_ACTION)
    }

    // 从url里面解析
    fn decode_common_headers_from_url<State>(
        req: &NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNGetDataUrlParams> {
        let mut flags = None;
        let mut dec_id = None;
        let mut level = None;
        let mut target = None;
        let mut referer_object = vec![];
        let mut inner_path = None;
        let mut action = None;
        let mut req_path = None;
        let mut object_id = None;
        let mut group = None;

        for (k, v) in req.request.url().query_pairs() {
            match k.as_ref() {
                cyfs_base::CYFS_OBJECT_ID => {
                    object_id = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                cyfs_base::CYFS_NDN_ACTION => {
                    action = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                cyfs_base::CYFS_FLAGS => {
                    flags = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                cyfs_base::CYFS_DEC_ID => {
                    dec_id = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                cyfs_base::CYFS_API_LEVEL => {
                    level = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                cyfs_base::CYFS_TARGET => {
                    target = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                cyfs_base::CYFS_REFERER_OBJECT => {
                    referer_object
                        .push(RequestorHelper::decode_url_param_with_utf8_decoding(k, v)?);
                }
                cyfs_base::CYFS_INNER_PATH => {
                    inner_path = Some(RequestorHelper::decode_url_param_with_utf8_decoding(k, v)?);
                }
                cyfs_base::CYFS_REQ_PATH => {
                    req_path = Some(RequestorHelper::decode_url_param_with_utf8_decoding(k, v)?);
                }
                cyfs_base::CYFS_TASK_GROUP => {
                    group = Some(RequestorHelper::decode_url_param_with_utf8_decoding(k, v)?);
                }
                _ => {
                    warn!("unknown ndn url param: {}={}", k, v);
                }
            }
        }

        if object_id.is_none() {
            let msg = format!(
                "invalid ndn download data request's object_id! url={}",
                req.request.url()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        // FIXME header dec_id vs query pairs dec_id?
        let source = req
            .source
            .clone()
            .dec(dec_id.unwrap_or(cyfs_core::get_anonymous_dec_app().to_owned()));

        let common = NDNInputRequestCommon {
            req_path,

            source,
            level: level.unwrap_or_default(),

            target,

            referer_object,

            flags: flags.unwrap_or(0),

            user_data: None,
        };

        let ret = NDNGetDataUrlParams {
            common,
            action,
            object_id: object_id.unwrap(),
            inner_path,
            group,
        };

        Ok(ret)
    }

    // 解析通用header字段
    fn decode_common_headers<State>(
        req: &NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNInputRequestCommon> {
        // req_path
        let req_path = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_REQ_PATH,
        )?;

        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 提取api level字段
        let level =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_API_LEVEL)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        // 提取关联对象
        let referer_object: Option<Vec<NDNDataRefererObject>> =
            RequestorHelper::decode_optional_headers_with_utf8_decoding(
                &req.request,
                cyfs_base::CYFS_REFERER_OBJECT,
            )?;

        let ret = NDNInputRequestCommon {
            req_path,

            source: req.source.clone(),
            level: level.unwrap_or_default(),
            target,

            referer_object: referer_object.unwrap_or(vec![]),

            flags: flags.unwrap_or(0),

            user_data: None,
        };

        Ok(ret)
    }

    pub fn encode_put_data_response(resp: NDNPutDataInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.insert_header(cyfs_base::CYFS_NDN_ACTION, &NDNAction::PutData.to_string());
        RequestorHelper::encode_header(&mut http_resp, cyfs_base::CYFS_RESULT, &resp.result);

        http_resp.into()
    }

    pub async fn process_put_data_request<State>(
        &self,
        req: NDNInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_put_data(req).await;
        match ret {
            Ok(resp) => Self::encode_put_data_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_put_data<State>(
        &self,
        mut req: NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNPutDataInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, NDNAction::PutData)?;
        if action != NDNAction::PutData && action != NDNAction::PutSharedData {
            let msg = format!("invalid ndn put_data action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let object_id = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_OBJECT_ID)?;

        // 提取body
        let data = req.request.take_body();

        // 必须要有content-length
        let length = data.len();
        if length.is_none() {
            let msg = format!("invalid non put_data content length!");
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let data = Box::new(data);

        let put_req = if action == NDNAction::PutData {
            NDNPutDataInputRequest {
                common,
                object_id,

                data_type: NDNDataType::Mem,
                length: length.unwrap() as u64,
                data,
            }
        } else {
            NDNPutDataInputRequest {
                common,
                object_id,

                data_type: NDNDataType::SharedMem,
                length: length.unwrap() as u64,
                data,
            }
        };

        info!("recv put_data request: {}", put_req);

        self.processor.put_data(put_req).await
    }

    pub fn encode_get_data_response(resp: NDNGetDataInputResponse) -> Response {
        let mut http_resp = match resp.range {
            Some(range) => {
                let mut resp = RequestorRangeHelper::new_range_response(&range);
                resp.insert_header(cyfs_base::CYFS_DATA_RANGE, range.encode_string());
                resp
            }
            None => RequestorHelper::new_response(StatusCode::Ok),
        };

        // resp里面增加action的具体类型，方便一些需要根据请求类型做二次处理的地方
        http_resp.insert_header(cyfs_base::CYFS_NDN_ACTION, &NDNAction::GetData.to_string());

        http_resp.insert_header(cyfs_base::CYFS_OBJECT_ID, resp.object_id.to_string());
        if let Some(owner_id) = &resp.owner_id {
            http_resp.insert_header(cyfs_base::CYFS_OWNER_ID, owner_id.to_string());
        }

        if let Some(attr) = &resp.attr {
            http_resp.insert_header(cyfs_base::CYFS_ATTRIBUTES, attr.flags().to_string());
        }

        RequestorHelper::encode_opt_header_with_encoding(
            &mut http_resp,
            cyfs_base::CYFS_TASK_GROUP,
            resp.group.as_deref(),
        );

        if http_resp.status().is_success() {
            let reader = BufReader::new(resp.data);
            let body = tide::Body::from_reader(reader, Some(resp.length as usize));
            http_resp.set_body(body);
        }

        http_resp.into()
    }

    pub async fn process_get_request<State>(&self, req: NDNInputHttpRequest<State>) -> Response {
        match Self::decode_option_action(&req) {
            Ok(Some(action)) => {
                if action == NDNAction::QueryFile {
                    self.process_query_file_request(req).await
                } else {
                    self.process_get_data_request(action, req).await
                }
            }
            Ok(None) => self.process_download_data_request(req).await,
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn process_get_data_request<State>(
        &self,
        action: NDNAction,
        req: NDNInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_data(action, req).await;
        match ret {
            Ok(resp) => Self::encode_get_data_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_data<State>(
        &self,
        action: NDNAction,
        req: NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNGetDataInputResponse> {
        if action != NDNAction::GetData && action != NDNAction::GetSharedData {
            let msg = format!("invalid ndn get_data action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let object_id = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_OBJECT_ID)?;
        let inner_path = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_INNER_PATH,
        )?;
        let group = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_TASK_GROUP,
        )?;

        // check if range header applied
        let range = RequestorHelper::decode_optional_header(&req.request, "Range")?
            .map(|s: String| NDNDataRequestRange::new_unparsed(s));

        let get_req = if action == NDNAction::GetData {
            NDNGetDataInputRequest {
                common,
                object_id,

                data_type: NDNDataType::Mem,
                range,
                inner_path,

                group,
            }
        } else {
            NDNGetDataInputRequest {
                common,
                object_id,

                data_type: NDNDataType::SharedMem,
                range,
                inner_path,

                group,
            }
        };

        info!("recv get_data request: {}", get_req);

        get_req.check_valid()?;

        self.processor.get_data(get_req).await
    }

    fn decode_query_request<State>(
        req: NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNQueryFileInputRequest> {
        let mut t = None;
        let mut value = None;

        for (k, v) in req.request.url().query_pairs() {
            match k.as_ref() {
                "type" => {
                    t = Some(v);
                }
                "value" => {
                    value = Some(v);
                }

                _ => {
                    warn!("unknown ndn query file param: {} = {}", k, v);
                }
            }
        }

        if t.is_none() || value.is_none() {
            let msg = format!(
                "invalid ndn query file param, type or value is not specified! url={}",
                req.request.url()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let param = NDNQueryFileParam::from_key_pair(&t.unwrap(), &value.unwrap())?;

        let common = Self::decode_common_headers(&req)?;

        let query_req = NDNQueryFileInputRequest { common, param };

        info!("recv query_file request: {}", query_req);
        Ok(query_req)
    }

    pub fn encode_query_file_response(resp: NDNQueryFileInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        // resp里面增加action的具体类型，方便一些需要根据请求类型做二次处理的地方
        http_resp.insert_header(
            cyfs_base::CYFS_NDN_ACTION,
            &NDNAction::QueryFile.to_string(),
        );

        let s = resp.encode_string();
        http_resp.set_content_type(::tide::http::mime::JSON);
        http_resp.set_body(s);

        http_resp.into()
    }

    async fn on_query_file<State>(
        &self,
        req: NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        let query_req = Self::decode_query_request(req)?;

        self.processor.query_file(query_req).await
    }

    async fn process_query_file_request<State>(&self, req: NDNInputHttpRequest<State>) -> Response {
        let ret = self.on_query_file(req).await;
        match ret {
            Ok(resp) => Self::encode_query_file_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    // get_data以download模式请求
    pub async fn process_download_data_request<State>(
        &self,
        req: NDNInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_download_data(req).await;
        match ret {
            Ok(resp) => Self::encode_get_data_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_download_data<State>(
        &self,
        req: NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNGetDataInputResponse> {
        let get_data_params = Self::decode_common_headers_from_url(&req)?;

        let action = get_data_params.action.unwrap_or(NDNAction::GetData);
        if action != NDNAction::GetData {
            let msg = format!("invalid ndn get_data action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = get_data_params.common;

        // check if range header applied
        let range = RequestorHelper::decode_optional_header(&req.request, "Range")?
            .map(|s: String| NDNDataRequestRange::new_unparsed(s));

        let get_req = NDNGetDataInputRequest {
            common,
            object_id: get_data_params.object_id,

            data_type: NDNDataType::Mem,
            range,
            inner_path: get_data_params.inner_path,
            group: get_data_params.group,
        };

        info!("recv get_data as download request: {}", get_req);

        self.processor.get_data(get_req).await
    }

    pub async fn process_delete_data_request<State>(
        &self,
        req: NDNInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_delete_data(req).await;
        match ret {
            Ok(resp) => Self::encode_delete_data_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub fn encode_delete_data_response(resp: NDNDeleteDataInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.insert_header(
            cyfs_base::CYFS_NDN_ACTION,
            &NDNAction::DeleteData.to_string(),
        );
        RequestorHelper::encode_header(&mut http_resp, cyfs_base::CYFS_OBJECT_ID, &resp.object_id);

        // FIXME 是否要返回buffer和delete_size这些？

        http_resp.into()
    }

    async fn on_delete_data<State>(
        &self,
        req: NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, NDNAction::DeleteData)?;
        if action != NDNAction::DeleteData {
            let msg = format!("invalid ndn delete_data action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let object_id = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_OBJECT_ID)?;
        let inner_path = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_INNER_PATH,
        )?;

        let delete_req = NDNDeleteDataInputRequest {
            common,
            object_id,
            inner_path,
        };

        info!("recv delete_data request: {}", delete_req);

        self.processor.delete_data(delete_req).await
    }
}
