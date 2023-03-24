use super::processor::*;
use super::request::*;
use crate::backup::*;
use cyfs_base::*;
use cyfs_bdt::ChunkReaderRef;
use cyfs_lib::*;

use std::sync::Arc;

pub struct BackupService {
    backup_manager: Option<BackupManagerRef>,
    restore_manager: Option<RestoreManagerRef>,
}

impl BackupService {
    pub fn new(
        noc: NamedObjectCacheRef,
        ndc: NamedDataCacheRef,
        chunk_reader: ChunkReaderRef,
    ) -> Self {
        let backup_manager = BackupManager::new(noc, ndc, chunk_reader);
        let restore_manager = RestoreManager::new();

        Self {
            backup_manager: Some(Arc::new(backup_manager)),
            restore_manager: Some(Arc::new(restore_manager)),
        }
    }

    pub fn new_direct(
        backup_manager: Option<BackupManagerRef>,
        restore_manager: Option<RestoreManagerRef>,
    ) -> Self {
        Self {
            backup_manager,
            restore_manager,
        }
    }

    pub fn into_processor(self) -> BackupInputProcessorRef {
        Arc::new(Box::new(self))
    }

    fn backup_manager(&self) -> BuckyResult<&BackupManagerRef> {
        self.backup_manager.as_ref().ok_or_else(|| {
            let msg = format!("backup manager not support!");
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::UnSupport, msg)
        })
    }

    fn restore_manager(&self) -> BuckyResult<&RestoreManagerRef> {
        self.restore_manager.as_ref().ok_or_else(|| {
            let msg = format!("restore manager not support!");
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::UnSupport, msg)
        })
    }
}

pub type BackupServiceRef = Arc<BackupService>;

#[async_trait::async_trait]
impl BackupInputProcessor for BackupService {
    async fn start_backup_task(
        &self,
        req: StartBackupTaskInputRequest,
    ) -> BuckyResult<StartBackupTaskInputResponse> {
        let result = self
            .backup_manager()?
            .start_uni_backup(req.request.params)
            .await;

        Ok(StartBackupTaskInputResponse { result })
    }

    async fn get_backup_task_status(
        &self,
        req: GetBackupTaskStatusInputRequest,
    ) -> BuckyResult<GetBackupTaskStatusInputResponse> {
        let status = self.backup_manager()?.get_task_status(&req.request.id)?;

        Ok(GetBackupTaskStatusInputResponse {
            id: req.request.id,
            status,
        })
    }
}
