use cyfs_base::*;
use cyfs_core::TransContext;
use cyfs_lib::*;
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
        match self.on_add_file(req).await {
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
                let body = resp.to_hex().unwrap();
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
        // let mut http_req = TransInputHttpRequest::new(&self.protocol, req);

        // 提取body里面的object对象，如果有的话
        let body = req.request.body_string().await.map_err(|e| {
            let msg = format!("trans start task failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransCreateTaskInputRequest::decode_string(&body)?;

        self.processor.create_task(req).await
    }

    async fn on_control_task<State>(&self, mut req: NONInputHttpRequest<State>) -> BuckyResult<()> {
        // let mut http_req = TransInputHttpRequest::new(&self.protocol, req);

        // 提取body里面的object对象，如果有的话
        let body = req.request.body_string().await.map_err(|e| {
            let msg = format!("trans control task failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransControlTaskInputRequest::decode_string(&body)?;
        self.processor.control_task(req).await
    }

    async fn on_get_task_state<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<TransTaskState> {
        // let mut http_req = TransInputHttpRequest::new(&self.protocol, req);
        // 提取body里面的object对象，如果有的话
        let body = req.request.body_string().await.map_err(|e| {
            let msg = format!("trans get task state failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransGetTaskStateInputRequest::decode_string(&body)?;

        self.processor.get_task_state(req).await
    }

    async fn on_add_file<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        // let mut http_req = TransInputHttpRequest::new(&self.protocol, req);

        // 提取body里面的object对象，如果有的话
        let body = req.request.body_string().await.map_err(|e| {
            let msg = format!("trans add file failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransPublishFileInputRequest::decode_string(&body)?;

        self.processor.publish_file(req).await
    }

    async fn on_get_context<State>(&self, mut req: NONInputHttpRequest<State>) -> BuckyResult<TransContext> {
        // let mut http_req = TransInputHttpRequest::new(&self.protocol, req);

        let body = req.request.body_string().await.map_err(|e| {
            let msg = format!("trans get context failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransGetContextInputRequest::decode_string(&body)?;
        self.processor.get_context(req).await
    }

    async fn on_update_context<State>(&self, mut req: NONInputHttpRequest<State>) -> BuckyResult<()> {
        // let mut http_req = TransInputHttpRequest::new(&self.protocol, req);

        let body = req.request.body_string().await.map_err(|e| {
            let msg = format!("trans get context failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransUpdateContextInputRequest::decode_string(&body)?;
        self.processor.put_context(req).await
    }

    async fn on_query_tasks_context<State>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        // let mut http_req = TransInputHttpRequest::new(&self.protocol, req);

        let body = req.request.body_string().await.map_err(|e| {
            let msg = format!("trans querty tasks failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransQueryTasksInputRequest::decode_string(&body)?;
        self.processor.query_tasks(req).await
    }
}
