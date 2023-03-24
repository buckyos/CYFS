use super::processor::BackupInputProcessorRef;
use super::request::*;
use cyfs_base::*;
use cyfs_lib::*;

use http_types::StatusCode;
use tide::Response;

#[derive(Clone)]
pub(crate) struct BackupRequestHandler {
    processor: BackupInputProcessorRef,
}

impl BackupRequestHandler {
    pub fn new(processor: BackupInputProcessorRef) -> Self {
        Self { processor }
    }

    pub async fn process_start_backup_task_request<State: Send>(
        &self,
        req: BackupInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_start_backup_task(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(serde_json::to_string(&resp).unwrap());

                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_start_backup_task<State>(
        &self,
        mut req: BackupInputHttpRequest<State>,
    ) -> BuckyResult<StartBackupTaskInputResponse> {
        let request = req.request.body_json().await.map_err(|e| {
            let msg = format!("read start_backup_task request from body failed! {}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        let request = StartBackupTaskInputRequest {
            source: req.source,
            request,
        };

        self.processor.start_backup_task(request).await
    }

    pub async fn process_get_backup_task_status_request<State: Send>(
        &self,
        req: BackupInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_start_backup_task(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

                http_resp.set_content_type(::tide::http::mime::JSON);
                http_resp.set_body(serde_json::to_string(&resp).unwrap());

                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_backup_task_status<State>(
        &self,
        mut req: BackupInputHttpRequest<State>,
    ) -> BuckyResult<GetBackupTaskStatusInputResponse> {
        let request = req.request.body_json().await.map_err(|e| {
            let msg = format!(
                "read get_backup_task_status request from body failed! {}",
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        let request = GetBackupTaskStatusInputRequest {
            source: req.source,
            request,
        };

        self.processor.get_backup_task_status(request).await
    }
}
