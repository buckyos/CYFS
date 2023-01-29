use super::output_request::*;
use crate::base::*;
use crate::*;
use cyfs_base::*;
use cyfs_core::TransContextObject;

use http_types::{Method, Request, Url};
use std::sync::Arc;

#[derive(Clone)]
pub struct TransRequestor {
    dec_id: Option<SharedObjectStackDecID>,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl TransRequestor {
    pub fn new(dec_id: Option<SharedObjectStackDecID>, requestor: HttpRequestorRef) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/trans/", addr);
        let url = Url::parse(&url).unwrap();

        Self {
            dec_id,
            requestor,
            service_url: url,
        }
    }

    pub fn clone_processor(&self) -> TransOutputProcessorRef {
        Arc::new(self.clone())
    }

    fn encode_common_headers(&self, com_req: &NDNOutputRequestCommon, http_req: &mut Request) {
        if let Some(dec_id) = &com_req.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        } else if let Some(dec_id) = &self.dec_id {
            if let Some(dec_id) = dec_id.get() {
                http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
            }
        }

        RequestorHelper::encode_opt_header_with_encoding(
            http_req,
            cyfs_base::CYFS_REQ_PATH,
            com_req.req_path.as_deref(),
        );
        http_req.insert_header(CYFS_API_LEVEL, com_req.level.to_string());

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        if !com_req.referer_object.is_empty() {
            RequestorHelper::insert_headers_with_encoding(
                http_req,
                cyfs_base::CYFS_REFERER_OBJECT,
                &com_req.referer_object,
            );
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());
    }

    pub async fn get_context(
        &self,
        req: TransGetContextOutputRequest,
    ) -> BuckyResult<TransGetContextOutputResponse> {
        info!(
            "will get context id={:?}, path={:?}",
            req.context_id, req.context_path
        );

        let url = self.service_url.join("get_context").unwrap();
        let mut http_req = Request::new(Method::Post, url);

        self.encode_common_headers(&req.common, &mut http_req);
        let body = req.encode_string();
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;
        match resp.status() {
            code if code.is_success() => {
                let context = RequestorHelper::decode_raw_object_body(&mut resp).await?;

                Ok(TransGetContextOutputResponse { context })
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "get context failed: id={:?}, path={:?}, status={}, {}",
                    req.context_id, req.context_path, code, e
                );

                Err(e)
            }
        }
    }

    pub async fn put_context(&self, req: TransPutContextOutputRequest) -> BuckyResult<()> {
        info!("will put context {}", req.context.context_path());

        let url = self.service_url.join("put_context").unwrap();
        let mut http_req = Request::new(Method::Post, url);

        self.encode_common_headers(&req.common, &mut http_req);

        if let Some(access) = &req.access {
            http_req.insert_header(cyfs_base::CYFS_ACCESS, access.value().to_string());
        }
        
        let body = req.context.to_vec()?;
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;
        match resp.status() {
            code if code.is_success() => Ok(()),
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "update context failed: context={}, status={}, {}",
                    req.context.context_path(),
                    code,
                    e
                );
                Err(e)
            }
        }
    }

    pub async fn create_task(
        &self,
        req: TransCreateTaskOutputRequest,
    ) -> BuckyResult<TransCreateTaskOutputResponse> {
        info!("will create trans task: {:?}", req);

        let url = self.service_url.join("task").unwrap();
        let mut http_req = Request::new(Method::Post, url);

        self.encode_common_headers(&req.common, &mut http_req);
        let body = req.encode_string();
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;

        match resp.status() {
            code if code.is_success() => {
                let body = resp.body_string().await.map_err(|e| {
                    let msg = format!(
                        "trans create task failed, read body string error! req={:?} {}",
                        req, e
                    );
                    error!("{}", msg);

                    BuckyError::from(msg)
                })?;

                let resp = TransCreateTaskOutputResponse::decode_string(&body).map_err(|e| {
                    error!(
                        "decode trans create task resp from body string error: body={} {}",
                        body, e,
                    );
                    e
                })?;

                debug!("trans create task success: resp={:?}", resp.task_id);

                Ok(resp)
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "create task failed: obj={}, status={}, {}",
                    req.object_id, code, e
                );
                Err(e)
            }
        }
    }

    pub async fn control_task(&self, req: TransControlTaskOutputRequest) -> BuckyResult<()> {
        info!("will control trans task: {:?}", req);

        let url = self.service_url.join("task").unwrap();
        let mut http_req = Request::new(Method::Put, url);

        self.encode_common_headers(&req.common, &mut http_req);
        let body = req.encode_string();
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;

        match resp.status() {
            code if code.is_success() => Ok(()),
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "stop trans task failed: task={}, status={}, {}",
                    req.task_id, code, e
                );
                Err(e)
            }
        }
    }

    pub async fn start_task(&self, req: TransTaskOutputRequest) -> BuckyResult<()> {
        Self::control_task(
            self,
            TransControlTaskOutputRequest {
                common: req.common.clone(),
                task_id: req.task_id.clone(),
                action: TransTaskControlAction::Start,
            },
        )
        .await
    }

    pub async fn stop_task(&self, req: TransTaskOutputRequest) -> BuckyResult<()> {
        Self::control_task(
            self,
            TransControlTaskOutputRequest {
                common: req.common.clone(),
                task_id: req.task_id.clone(),
                action: TransTaskControlAction::Stop,
            },
        )
        .await
    }

    pub async fn delete_task(&self, req: TransTaskOutputRequest) -> BuckyResult<()> {
        Self::control_task(
            self,
            TransControlTaskOutputRequest {
                common: req.common.clone(),
                task_id: req.task_id.clone(),
                action: TransTaskControlAction::Delete,
            },
        )
        .await
    }

    pub async fn get_task_state(
        &self,
        req: TransGetTaskStateOutputRequest,
    ) -> BuckyResult<TransGetTaskStateOutputResponse> {
        info!("will get trans task state: {:?}", req);

        let url = self.service_url.join("task/state").unwrap();
        let mut http_req = Request::new(Method::Get, url);

        self.encode_common_headers(&req.common, &mut http_req);
        let body = req.encode_string();
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;

        match resp.status() {
            code if code.is_success() => {
                let content = resp.body_json().await.map_err(|e| {
                    let msg = format!("parse TransTaskState resp body error! err={}", e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

                info!(
                    "got trans task state: task={}, state={:?}",
                    req.task_id, content
                );

                Ok(content)
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "get trans task state failed: task={}, status={}, {}",
                    req.task_id, code, e,
                );
                Err(e)
            }
        }
    }

    pub async fn query_tasks(
        &self,
        req: TransQueryTasksOutputRequest,
    ) -> BuckyResult<TransQueryTasksOutputResponse> {
        info!("will query tasks: {:?}", req);

        let url = self.service_url.join("tasks").unwrap();
        let mut http_req = Request::new(Method::Post, url);

        self.encode_common_headers(&req.common, &mut http_req);
        let body = req.encode_string();
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;

        match resp.status() {
            code if code.is_success() => {
                let content = resp.body_string().await.map_err(|e| {
                    let msg = format!("get query task resp body error! err={}", e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

                let resp = TransQueryTasksOutputResponse::decode_string(content.as_str())?;
                Ok(resp)
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!("query tasks failed: status={}, msg={}", code, e);

                Err(e)
            }
        }
    }

    pub async fn publish_file(
        &self,
        req: TransPublishFileOutputRequest,
    ) -> BuckyResult<TransPublishFileOutputResponse> {
        info!("will publish file: {:?}", req);

        let url = self.service_url.join("file").unwrap();
        let mut http_req = Request::new(Method::Post, url);

        self.encode_common_headers(&req.common, &mut http_req);
        let body = req.encode_string();
        http_req.set_body(body);

        let mut resp = self.requestor.request(http_req).await?;

        match resp.status() {
            code if code.is_success() => {
                let body = resp.body_string().await.map_err(|e| {
                    let msg = format!(
                        "trans publish file failed, read body string error! req={:?} {}",
                        req, e
                    );
                    error!("{}", msg);

                    BuckyError::from(msg)
                })?;

                let resp = TransPublishFileOutputResponse::decode_string(&body).map_err(|e| {
                    error!(
                        "decode trans publish file resp from body string error: body={} {}",
                        body, e,
                    );
                    e
                })?;

                debug!("trans publish file success: resp={:?}", resp);

                Ok(resp)
            }
            code @ _ => {
                let e = RequestorHelper::error_from_resp(&mut resp).await;
                error!(
                    "trans publish file failed: file={}, status={}, {}",
                    req.local_path.display(),
                    code,
                    e
                );

                Err(e)
            }
        }
    }

    pub async fn get_task_group_state(
        &self,
        req: TransGetTaskGroupStateOutputRequest,
    ) -> BuckyResult<TransGetTaskGroupStateOutputResponse> {
        info!("will get trans task group state: {:?}", req);

        let url = self.service_url.join("task_group/state").unwrap();
        let mut http_req = Request::new(Method::Post, url);

        self.encode_common_headers(&req.common, &mut http_req);
        http_req.set_body(serde_json::to_string(&req).unwrap());

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let content = resp.body_json().await.map_err(|e| {
                let msg = format!("parse get task group state resp body error! err={}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            info!(
                "got trans task group state: task_group={}, state={:?}",
                req.group, content
            );

            Ok(content)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "get trans task state failed: task_group={}, status={}, {}",
                req.group,
                resp.status(),
                e
            );

            Err(e)
        }
    }

    pub async fn control_task_group(
        &self,
        req: TransControlTaskGroupOutputRequest,
    ) -> BuckyResult<TransControlTaskGroupOutputResponse> {
        info!("will control trans task group: {:?}", req);

        let url = self.service_url.join("task_group").unwrap();
        let mut http_req = Request::new(Method::Put, url);

        self.encode_common_headers(&req.common, &mut http_req);
        http_req.set_body(serde_json::to_string(&req).unwrap());

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = resp.body_json().await.map_err(|e| {
                let msg = format!(
                    "trans control task group failed, read body string error! req={:?} {}",
                    req, e
                );
                error!("{}", msg);

                BuckyError::from(msg)
            })?;

            debug!("trans control task group success: resp={:?}", resp);

            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("trans control task failed! status={}, {}", resp.status(), e);

            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl TransOutputProcessor for TransRequestor {
    async fn get_context(
        &self,
        req: TransGetContextOutputRequest,
    ) -> BuckyResult<TransGetContextOutputResponse> {
        Self::get_context(self, req).await
    }

    async fn put_context(&self, req: TransPutContextOutputRequest) -> BuckyResult<()> {
        Self::put_context(self, req).await
    }

    async fn create_task(
        &self,
        req: TransCreateTaskOutputRequest,
    ) -> BuckyResult<TransCreateTaskOutputResponse> {
        Self::create_task(self, req).await
    }

    async fn query_tasks(
        &self,
        req: TransQueryTasksOutputRequest,
    ) -> BuckyResult<TransQueryTasksOutputResponse> {
        Self::query_tasks(self, req).await
    }

    async fn get_task_state(
        &self,
        req: TransGetTaskStateOutputRequest,
    ) -> BuckyResult<TransGetTaskStateOutputResponse> {
        Self::get_task_state(self, req).await
    }

    async fn publish_file(
        &self,
        req: TransPublishFileOutputRequest,
    ) -> BuckyResult<TransPublishFileOutputResponse> {
        Self::publish_file(self, req).await
    }

    async fn control_task(&self, req: TransControlTaskOutputRequest) -> BuckyResult<()> {
        Self::control_task(self, req).await
    }

    async fn get_task_group_state(
        &self,
        req: TransGetTaskGroupStateOutputRequest,
    ) -> BuckyResult<TransGetTaskGroupStateOutputResponse> {
        Self::get_task_group_state(self, req).await
    }

    async fn control_task_group(
        &self,
        req: TransControlTaskGroupOutputRequest,
    ) -> BuckyResult<TransControlTaskGroupOutputResponse> {
        Self::control_task_group(self, req).await
    }
}
/*
struct TransHelper {

}

impl TransHelper {
    pub async fn download_chunk_sync(requestor: &TransRequestor, chunk_id: ChunkId, device_id: DeviceId) -> BuckyResult<Vec<u8>> {

        let local_path= cyfs_util::get_temp_path().join("trans_chunk").join(chunk_id.to_string());

        // 创建下载任务
        let req = TransStartTaskRequest {
            target: None,
            object_id: chunk_id.object_id().to_owned(),
            local_path: local_path.clone(),
            device_list: vec![device_id.clone()],
        };

        info!("will download chunk to tmp, chunk_id={}, tmp_file={}", chunk_id, local_path.display());

        requestor.start_task(&req).await.map_err(|e|{
            error!("trans start task error! chunk_id={}, {}", chunk_id, e);
            e
        })?;

        loop {
            let req = TransGetTaskStateRequest {
                target: None,
                object_id: chunk_id.object_id().to_owned(),
                local_path: local_path.clone(),
            };

            let state = requestor.get_task_state(&req).await.map_err(|e| {
                error!("get trans task state error! chunk={}, {}", chunk_id, e);
                e
            })?;

            match state {
                TransTaskState::Downloading(v) => {
                    // info!("trans task downloading! file_id={}, {:?}", chunk_id, v);
                }
                TransTaskState::Finished(_v) => {
                    info!("chunk trans task finished! chunk_id={}", chunk_id);
                    break;
                }
                TransTaskState::Canceled | TransTaskState::Paused | TransTaskState::Pending => {
                    unreachable!()
                }
            }

            async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        }

        let mut f = async_std::fs::OpenOptions::new().read(true).open(&local_path).await.unwrap();
        let mut buf = vec![];
        let bytes = f.read_to_end(&mut buf).await.unwrap();
        if let Err(e) = async_std::fs::remove_file(&local_path).await {
            error!("remove tmp chunk file error!")
        }

        if bytes != chunk_id.len() {

        }
    }
}
*/
