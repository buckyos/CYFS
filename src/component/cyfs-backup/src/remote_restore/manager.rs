use super::def::*;
use super::status::*;
use super::task::*;
use cyfs_base::*;

use std::sync::{Arc, Mutex};

pub struct RemoteRestoreManager {
    // all restore tasks
    tasks: Mutex<Vec<RemoteRestoreTask>>,
}

impl RemoteRestoreManager {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(vec![]),
        }
    }

    fn create_remote_restore_task(
        &self,
        params: &RemoteRestoreParams,
    ) -> BuckyResult<RemoteRestoreTask> {
        let task = RemoteRestoreTask::new(params.id.clone());

        {
            let mut tasks = self.tasks.lock().unwrap();
            if tasks.iter().find(|item| item.id() == params.id).is_some() {
                let msg = format!("remote restore task already exists! task={}", params.id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
            }

            tasks.push(task.clone());
        }

        Ok(task)
    }

    pub async fn run_remote_restore(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        let task = self.create_remote_restore_task(&params)?;

        task.run(params).await
    }

    pub async fn start_remote_restore(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        let task = self.create_remote_restore_task(&params)?;

        async_std::task::spawn(async move {
            let id = params.id.clone();
            match task.run(params).await {
                Ok(()) => {
                    info!("run remote restore task complete! task={}", id);
                }
                Err(e) => {
                    error!("run remote restore task failed! task={}, {}", id, e);
                }
            }
        });

        Ok(())
    }

    pub fn get_task_status(&self, id: &str) -> BuckyResult<RemoteRestoreStatus> {
        let status = {
            let tasks = self.tasks.lock().unwrap();
            let ret = tasks.iter().find(|item| item.id() == id);
            if ret.is_none() {
                let msg = format!("backup task not exists! task={}", id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }

            ret.unwrap().status()
        };

        Ok(status)
    }
}

pub type RemoteRestoreManagerRef = Arc<RemoteRestoreManager>;
