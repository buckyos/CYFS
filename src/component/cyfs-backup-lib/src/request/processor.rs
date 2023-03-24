use super::request::*;
use cyfs_base::*;

use std::sync::Arc;


#[async_trait::async_trait]
pub trait BackupOutputProcessor: Sync + Send + 'static {
    async fn start_backup_task(
        &self,
        req: StartBackupTaskRequest,
    ) -> BuckyResult<StartBackupTaskResponse>;

    async fn get_backup_task_status(&self, req: GetBackupTaskStatusRequest)
        -> BuckyResult<GetBackupTaskStatusResponse>;
}

pub type BackupOutputProcessorRef = Arc<Box<dyn BackupOutputProcessor>>;

