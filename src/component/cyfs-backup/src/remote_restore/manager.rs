use super::task::*;
use cyfs_backup_lib::*;
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

    pub fn get_tasks(&self) -> Vec<String> {
        let tasks = self.tasks.lock().unwrap();
        let list: Vec<String> = tasks.iter().map(|task| task.id().to_owned()).collect();

        list
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

    pub fn start_remote_restore(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        let mut task = self.create_remote_restore_task(&params)?;

        task.start(params)
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

    pub async fn abort_task(&self, id: &str) -> BuckyResult<()> {
        let task = {
            let mut tasks = self.tasks.lock().unwrap();
            if let Some(index) = tasks.iter().position(|task| task.id() == id) {
                tasks.swap_remove(index)
            } else {
                let msg = format!("backup task not exists! task={}", id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        };

        task.abort().await;

        Ok(())
    }
}

pub type RemoteRestoreManagerRef = Arc<RemoteRestoreManager>;
