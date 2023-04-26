use cyfs_backup::*;
use cyfs_base::*;
use cyfs_lib::RequestorHelper;

use tide::{Response, Request, StatusCode};

pub struct RestoreController  {
    restore_manager: RemoteRestoreManager,
}

impl RestoreController {
    pub fn new() -> Self {
        Self {
            restore_manager: RemoteRestoreManager::new(),
        }
    }

    pub async fn process_create_remote_restore_task_request(&self, mut req: Request<()>) -> tide::Response {
        match req.body_json().await {
            Ok(param) => {
                match self.start_remote_restore(param) {
                    Ok(()) => {
                        RequestorHelper::new_ok_response()
                    }
                    Err(e) => {
                        RequestorHelper::trans_error(e)
                    }
                }
            }
            Err(e) => {
                let msg = format!("parse restore params error: {}", e);
                error!("{}", msg);

                Response::builder(StatusCode::BadRequest).body(msg).build()
            }
        }
    }

    fn start_remote_restore(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        self.restore_manager.start_remote_restore(params)
    }

    pub async fn process_get_remote_restore_task_status_request(&self, req: Request<()>) -> tide::Response {
        match self.get_task_status(req) {
            Ok(status) => {
                let mut resp: Response = RequestorHelper::new_ok_response();
                let body = serde_json::to_string(&status).unwrap();

                resp.set_content_type(tide::http::mime::JSON);
                resp.set_body(body);

                resp
            }
            Err(e) => {
                let msg = format!("parse restore task_id error: {}", e);
                error!("{}", msg);

                Response::builder(StatusCode::BadRequest).body(msg).build()
            }
        }
    }

    pub fn get_task_status(&self, req: Request<()>) -> BuckyResult<RemoteRestoreStatus> {
        let task_id = req.param("task_id") .map_err(|e| {
            let msg = format!("invalid task_id segment: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        self.restore_manager.get_task_status(task_id)
    }
}