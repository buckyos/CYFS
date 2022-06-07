use cyfs_base::*;
use cyfs_core::TransContext;
use cyfs_lib::*;

use crate::non::NONInputHttpRequest;
use crate::trans::{TransInputProcessorRef, TransOutputTransformer};

#[derive(Clone)]
pub(crate) struct TransRequestHandler {
    protocol: NONProtocol,
    processor: TransInputProcessorRef,
}

impl TransRequestHandler {
    pub fn new(
        protocol: NONProtocol,
        processor: TransInputProcessorRef,
    ) -> Self {
        Self {
            protocol,
            processor,
        }
    }

    fn get_processor<State>(&self, req: &NONInputHttpRequest<State>) -> TransOutputProcessorRef {
        TransOutputTransformer::new(self.processor.clone(), req.source.clone(), Some(self.protocol.clone()))
    }

    // trans/start
    pub async fn process_create_task<State>(&self, req: tide::Request<State>) -> tide::Response {
        match self.on_create_task(req).await {
            Ok(resp) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();
                http_resp.set_body(resp.encode_string());
                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_control_task<State>(&self, req: tide::Request<State>) -> tide::Response {
        match self.on_control_task(req).await {
            Ok(()) => {
                let http_resp: tide::Response = RequestorHelper::new_ok_response();

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_get_task_state<State>(&self, req: tide::Request<State>) -> tide::Response {
        match self.on_get_task_state(req).await {
            Ok(state) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();

                let body = serde_json::to_string(&state).unwrap();
                http_resp.set_body(body);

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_publish_file<State>(&self, req: tide::Request<State>) -> tide::Response {
        match self.on_add_file(req).await {
            Ok(resp) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();

                let body = resp.encode_string();
                http_resp.set_body(body);
                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_get_context<State>(&self, req: tide::Request<State>) -> tide::Response {
        match self.on_get_context(req).await {
            Ok(resp) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();
                let body = resp.to_hex().unwrap();
                http_resp.set_body(body);
                http_resp
            },
            Err(e) => RequestorHelper::trans_error(e)
        }
    }

    pub async fn process_put_context<State>(&self, req: tide::Request<State>) -> tide::Response {
        match self.on_update_context(req).await {
            Ok(()) => {
                let http_resp: tide::Response = RequestorHelper::new_ok_response();

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub async fn process_query_tasks_context<State>(&self, req: tide::Request<State>) -> tide::Response {
        match self.on_query_tasks_context(req).await {
            Ok(resp) => {
                let mut http_resp: tide::Response = RequestorHelper::new_ok_response();

                let body = resp.encode_string();
                http_resp.set_body(body);

                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_create_task<State>(&self, req: tide::Request<State>) -> BuckyResult<TransCreateTaskOutputResponse> {
        let mut http_req = NONInputHttpRequest::new(&self.protocol, req);

        // 提取body里面的object对象，如果有的话
        let body = http_req.request.body_string().await.map_err(|e| {
            let msg = format!("trans start task failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransCreateTaskOutputRequest::decode_string(&body)?;

        self.get_processor(&http_req).create_task(&req).await
    }

    async fn on_control_task<State>(&self, req: tide::Request<State>) -> BuckyResult<()> {
        let mut http_req = NONInputHttpRequest::new(&self.protocol, req);

        // 提取body里面的object对象，如果有的话
        let body = http_req.request.body_string().await.map_err(|e| {
            let msg = format!("trans control task failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransControlTaskOutputRequest::decode_string(&body)?;

        match req.action {
            TransTaskControlAction::Stop => {
                self.get_processor(&http_req).stop_task(&TransTaskOutputRequest {common: req.common, task_id: req.task_id}).await
            },
            TransTaskControlAction::Start => {
                self.get_processor(&http_req).start_task(&TransTaskOutputRequest {common: req.common, task_id: req.task_id}).await
            },
            TransTaskControlAction::Delete => {
                self.get_processor(&http_req).delete_task(&TransTaskOutputRequest {common: req.common, task_id: req.task_id}).await
            },
        }
    }

    async fn on_get_task_state<State>(
        &self,
        req: tide::Request<State>,
    ) -> BuckyResult<TransTaskState> {
        let mut http_req = NONInputHttpRequest::new(&self.protocol, req);
        // 提取body里面的object对象，如果有的话
        let body = http_req.request.body_string().await.map_err(|e| {
            let msg = format!("trans get task state failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransGetTaskStateOutputRequest::decode_string(&body)?;

        self.get_processor(&http_req).get_task_state(&req).await
    }

    async fn on_add_file<State>(
        &self,
        req: tide::Request<State>,
    ) -> BuckyResult<TransPublishFileOutputResponse> {
        let mut http_req = NONInputHttpRequest::new(&self.protocol, req);

        // 提取body里面的object对象，如果有的话
        let body = http_req.request.body_string().await.map_err(|e| {
            let msg = format!("trans add file failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransPublishFileOutputRequest::decode_string(&body)?;

        self.get_processor(&http_req).publish_file(&req).await
    }

    async fn on_get_context<State>(&self, req: tide::Request<State>) -> BuckyResult<TransContext> {
        let mut http_req = NONInputHttpRequest::new(&self.protocol, req);

        let body = http_req.request.body_string().await.map_err(|e| {
            let msg = format!("trans get context failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransGetContextOutputRequest::decode_string(&body)?;
        self.get_processor(&http_req).get_context(&req).await
    }

    async fn on_update_context<State>(&self, req: tide::Request<State>) -> BuckyResult<()> {
        let mut http_req = NONInputHttpRequest::new(&self.protocol, req);

        let body = http_req.request.body_string().await.map_err(|e| {
            let msg = format!("trans get context failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransPutContextOutputRequest::decode_string(&body)?;
        self.get_processor(&http_req).put_context(&req).await
    }

    async fn on_query_tasks_context<State>(&self, req: tide::Request<State>) -> BuckyResult<TransQueryTasksOutputResponse> {
        let mut http_req = NONInputHttpRequest::new(&self.protocol, req);

        let body = http_req.request.body_string().await.map_err(|e| {
            let msg = format!("trans querty tasks failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = TransQueryTasksOutputRequest::decode_string(&body)?;
        self.get_processor(&http_req).query_tasks(&req).await
    }
}
