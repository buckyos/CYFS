use cyfs_base::*;
use cyfs_lib::*;
use crate::ndn_api::NDNInputHttpRequest;
use crate::non::NONInputHttpRequest;

use crate::trans::TransInputProcessorRef;

#[derive(Clone)]
pub(crate) struct TransRequestHandler {
    processor: TransInputProcessorRef,
}

impl TransRequestHandler {
    pub fn new(processor: TransInputProcessorRef) -> Self {
        Self {
            processor,
        }
    }

    fn decode_common_headers<State>(
        req: &NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNInputRequestCommon> {
        // req_path
        let req_path =
            RequestorHelper::decode_optional_header_with_utf8_decoding(&req.request, cyfs_base::CYFS_REQ_PATH)?;

        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let level = RequestorHelper::decode_header(&req.request, CYFS_API_LEVEL)?;
        let referer_object = RequestorHelper::decode_optional_headers_with_utf8_decoding(&req.request, CYFS_REFERER_OBJECT)?.unwrap_or(vec![]);

        let ret = NDNInputRequestCommon {
            req_path,
            source: req.source.clone(),
            level,
            referer_object,
            target,
            flags: flags.unwrap_or(0),
            user_data: None
        };

        Ok(ret)
    }

    // trans/start
    pub async fn process_create_task<State>(&self, req: NONInputHttpRequest<State>,) -> tide::Response {
        match self.on_create_task(req).await {
            Ok(resp) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();
                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(resp.encode_string());
                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_control_task<State>(&self, req: NONInputHttpRequest<State>) -> tide::Response {
        match self.on_control_task(req).await {
            Ok(()) => {
                let http_resp: tide::Response = RequestorHelper::new_ok_response();

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_get_task_state<State>(&self, req: NONInputHttpRequest<State>) -> tide::Response {
        match self.on_get_task_state(req).await {
            Ok(state) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();

                let body = serde_json::to_string(&state).unwrap();
                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(body);

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_publish_file<State>(&self, req: NONInputHttpRequest<State>) -> tide::Response {
        match self.on_publish_file(req).await {
            Ok(resp) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();

                let body = resp.encode_string();
                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(body);
                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_get_context<State>(&self, req: NONInputHttpRequest<State>) -> tide::Response {
        match self.on_get_context(req).await {
            Ok(resp) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();
                let body = resp.context.to_vec().unwrap();
                http_resp.set_body(body);
                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_put_context<State>(&self, req: NONInputHttpRequest<State>) -> tide::Response {
        match self.on_update_context(req).await {
            Ok(()) => {
                let http_resp: tide::Response = RequestorHelper::new_ok_response();

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_query_tasks_context<State>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> tide::Response {
        match self.on_query_tasks_context(req).await {
            Ok(resp) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();

                let body = resp.encode_string();
                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(body);

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_create_task<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<TransCreateTaskInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        // 提取body里面的object对象，如果有的话
        let body = req.request.body_json().await.map_err(|e| {
            let msg = format!("trans start task failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransCreateTaskInputRequest {
            common,
            object_id: JsonCodecHelper::decode_string_field(&body, "object_id")?,
            local_path: JsonCodecHelper::decode_string_field(&body, "local_path")?,
            device_list: JsonCodecHelper::decode_str_array_field(&body, "device_list")?,
            group: JsonCodecHelper::decode_option_string_field(&body, "group")?,
            context: JsonCodecHelper::decode_option_string_field(&body, "context")?,
            auto_start: JsonCodecHelper::decode_bool_field(&body, "auto_start")?
        };

        req.check_valid()?;
        
        self.processor.create_task(req).await
    }

    async fn on_control_task<State>(&self, mut req: NONInputHttpRequest<State>) -> BuckyResult<()> {
        let common = Self::decode_common_headers(&req)?;

        // 提取body里面的object对象，如果有的话
        let body = req.request.body_json().await.map_err(|e| {
            let msg = format!("trans control task failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransControlTaskInputRequest{
            common,
            task_id: JsonCodecHelper::decode_string_field(&body, "task_id")?,
            action: JsonCodecHelper::decode_string_field(&body, "action")?,
        };
        self.processor.control_task(req).await
    }

    async fn on_get_task_state<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<TransGetTaskStateInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        // 提取body里面的object对象，如果有的话
        let body = req.request.body_json().await.map_err(|e| {
            let msg = format!("trans get task state failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransGetTaskStateInputRequest {
            common,
            task_id: JsonCodecHelper::decode_string_field(&body, "task_id")?,
        };

        self.processor.get_task_state(req).await
    }

    async fn on_publish_file<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        // 提取body里面的object对象，如果有的话
        let body = req.request.body_json().await.map_err(|e| {
            let msg = format!("trans publish file failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let access: Option<u32> = JsonCodecHelper::decode_option_int_field(&body, "access")?;
        let access = access.map(|v| AccessString::new(v));

        let req = TransPublishFileInputRequest{
            common,
            owner: JsonCodecHelper::decode_string_field(&body, "owner")?,
            local_path: JsonCodecHelper::decode_string_field(&body, "local_path")?,
            chunk_size: JsonCodecHelper::decode_int_field(&body, "chunk_size")?,
            file_id: JsonCodecHelper::decode_option_string_field(&body, "file_id")?,
            dirs: JsonCodecHelper::decode_option_array_field(&body, "dirs")?,
            access,
        };

        self.processor.publish_file(req).await
    }

    async fn on_get_context<State>(&self, mut req: NONInputHttpRequest<State>) -> BuckyResult<TransGetContextInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        let body = req.request.body_json().await.map_err(|e| {
            let msg = format!("trans get context failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransGetContextInputRequest {
            common,
            context_id: JsonCodecHelper::decode_option_string_field(&body, "context_id")?,
            context_path: JsonCodecHelper::decode_option_string_field(&body, "context_path")?
        };
        self.processor.get_context(req).await
    }

    async fn on_update_context<State>(&self, req: NONInputHttpRequest<State>) -> BuckyResult<()> {
        let common = Self::decode_common_headers(&req)?;

        let mut req: http_types::Request = req.request.into();
        let context = RequestorHelper::decode_raw_object_body(&mut req).await?;

        let access: Option<u32>= RequestorHelper::decode_optional_header(&req, cyfs_base::CYFS_ACCESS)?;
        let access = access.map(|v| AccessString::new(v));

        let req = TransUpdateContextInputRequest {
            common,
            context,
            access,
        };
        self.processor.put_context(req).await
    }

    async fn on_query_tasks_context<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        let body = req.request.body_json().await.map_err(|e| {
            let msg = format!("trans querty tasks failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let offset = JsonCodecHelper::decode_option_string_field(&body, "offset")?;
        let length = JsonCodecHelper::decode_option_string_field(&body, "length")?;
        let range = if offset.is_some() && length.is_some() {
            Some((offset.unwrap(), length.unwrap()))
        } else {
            None
        };

        let req = TransQueryTasksInputRequest {
            common,
            task_status: JsonCodecHelper::decode_option_string_field(&body, "task_status")?,
            range
        };
        self.processor.query_tasks(req).await
    }

    // task group
    pub async fn process_control_task_group<State>(&self, req: NONInputHttpRequest<State>) -> tide::Response {
        match self.on_control_task_group_state(req).await {
            Ok(resp) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();

                let body = serde_json::to_string(&resp).unwrap();
                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(body);

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_get_task_group_state<State>(&self, req: NONInputHttpRequest<State>) -> tide::Response {
        match self.on_get_task_group_state(req).await {
            Ok(state) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();

                let body = serde_json::to_string(&state).unwrap();
                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(body);

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_control_task_group_state<State>(&self, mut req: NONInputHttpRequest<State>) -> BuckyResult<TransControlTaskGroupInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        // 提取body里面的object对象，如果有的话
        let body = req.request.body_json().await.map_err(|e| {
            let msg = format!("trans control task group failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransControlTaskGroupInputRequest{
            common,
            group_type: JsonCodecHelper::decode_string_field(&body, "group_type")?,
            group: JsonCodecHelper::decode_string_field(&body, "group")?,
            action: JsonCodecHelper::decode_serde_field(&body, "action")?,
        };
        self.processor.control_task_group(req).await
    }

    async fn on_get_task_group_state<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<TransGetTaskGroupStateInputResponse> {
        let common = Self::decode_common_headers(&req)?;
        // 提取body里面的object对象，如果有的话
        let body = req.request.body_json().await.map_err(|e| {
            let msg = format!("trans get task group state failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransGetTaskGroupStateInputRequest {
            common,
            group_type: JsonCodecHelper::decode_string_field(&body, "group_type")?,
            group: JsonCodecHelper::decode_string_field(&body, "group")?,
            speed_when: JsonCodecHelper::decode_option_int_field(&body, "speed_when")?,
        };

        self.processor.get_task_group_state(req).await
    }
}
