use super::uni_restore_task::*;
use cyfs_backup_lib::*;
use cyfs_base::*;

use std::sync::{Arc, Mutex};

pub struct RestoreManager {
    // all restore tasks
    tasks: Mutex<Vec<UniRestoreTask>>,
}

impl RestoreManager {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(vec![]),
        }
    }

    fn create_uni_restore_task(&self, params: &UniRestoreParams) -> BuckyResult<UniRestoreTask> {
        let task = UniRestoreTask::new(params.id.clone());

        {
            let mut tasks = self.tasks.lock().unwrap();
            if tasks.iter().find(|item| item.id() == params.id).is_some() {
                let msg = format!("restore task already exists! task={}", params.id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
            }

            tasks.push(task.clone());
        }

        Ok(task)
    }

    pub async fn run_uni_restore(&self, params: UniRestoreParams) -> BuckyResult<()> {
        let task = self.create_uni_restore_task(&params)?;

        task.run(params).await
    }

    pub async fn start_uni_restore(&self, params: UniRestoreParams) -> BuckyResult<()> {
        let task = self.create_uni_restore_task(&params)?;

        async_std::task::spawn(async move {
            let id = params.id.clone();
            match task.run(params).await {
                Ok(()) => {
                    info!("run uni restore task complete! task={}", id);
                }
                Err(e) => {
                    error!("run uni restore task failed! task={}, {}", id, e);
                }
            }
        });

        Ok(())
    }

    pub fn get_task_status(&self, id: &str) -> BuckyResult<RestoreStatus> {
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

pub type RestoreManagerRef = Arc<RestoreManager>;
