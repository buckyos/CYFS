use cyfs_backup::*;
use cyfs_backup_lib::*;
use cyfs_base::*;
use cyfs_lib::RequestorHelper;

use tide::{Request, Response, StatusCode};

pub struct RestoreController {
    restore_manager: RemoteRestoreManager,
}

impl RestoreController {
    pub fn new() -> Self {
        Self {
            restore_manager: RemoteRestoreManager::new(),
        }
    }

    pub async fn process_create_remote_restore_task_request(
        &self,
        mut req: Request<()>,
    ) -> tide::Response {
        match req.body_json().await {
            Ok(param) => match self.start_remote_restore(param) {
                Ok(()) => RequestorHelper::new_ok_response(),
                Err(e) => RequestorHelper::trans_error(e),
            },
            Err(e) => {
                let msg = format!("parse restore params error: {}", e);
                error!("{}", msg);

                Response::builder(StatusCode::BadRequest).body(msg).build()
            }
        }
    }

    fn start_remote_restore(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        // In terms of ood-control, there is currently one and only one restore task in existence!
        let list = self.restore_manager.get_tasks();
        if !list.is_empty() {
            let msg = format!("restore task already exists: {:?}", list);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
        }

        self.restore_manager.start_remote_restore(params)
    }

    pub fn process_get_remote_restore_task_status_request(
        &self,
        req: Request<()>,
    ) -> tide::Response {
        match self.get_task_status(req) {
            Ok(status) => {
                let mut resp: Response = RequestorHelper::new_ok_response();
                let body = serde_json::to_string(&status).unwrap();

                resp.set_content_type(tide::http::mime::JSON);
                resp.set_body(body);

                resp
            }
            Err(e) => {
                RequestorHelper::trans_error(e)
            }
        }
    }

    fn get_task_status(&self, req: Request<()>) -> BuckyResult<RemoteRestoreStatus> {
        let task_id = req.param("task_id").map_err(|e| {
            let msg = format!("invalid task_id segment: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        self.restore_manager.get_task_status(task_id)
    }

    pub async fn process_abort_remote_restore_task_request(
        &self,
        req: Request<()>,
    ) -> tide::Response {
        match self.abort_task(req).await {
            Ok(status) => {
                let mut resp: Response = RequestorHelper::new_ok_response();
                let body = serde_json::to_string(&status).unwrap();

                resp.set_content_type(tide::http::mime::JSON);
                resp.set_body(body);

                resp
            }
            Err(e) => {
                RequestorHelper::trans_error(e)
            }
        }
    }

    async fn abort_task(&self, req: Request<()>) -> BuckyResult<()> {
        let task_id = req.param("task_id").map_err(|e| {
            let msg = format!("invalid task_id segment: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        self.restore_manager.abort_task(task_id).await
    }

    pub fn process_get_remote_restore_task_list_request(
        &self,
        _req: Request<()>,
    ) -> tide::Response {
        let list = self.restore_manager.get_tasks();

        let mut resp: Response = RequestorHelper::new_ok_response();
        let body = serde_json::to_string(&list).unwrap();

        resp.set_content_type(tide::http::mime::JSON);
        resp.set_body(body);

        resp
    }
}
