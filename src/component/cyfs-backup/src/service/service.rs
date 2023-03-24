use super::processor::*;
use super::request::*;
use crate::backup::*;
use cyfs_base::*;
use cyfs_bdt::ChunkReaderRef;
use cyfs_lib::*;

use std::sync::Arc;

pub struct BackupService {
    backup_manager: BackupManager,
    restore_manager: RestoreManager,
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
            backup_manager,
            restore_manager,
        }
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
            .backup_manager
            .start_uni_backup(req.request.params)
            .await;

        Ok(StartBackupTaskInputResponse { result })
    }

    async fn get_backup_task_status(
        &self,
        req: GetBackupTaskStatusInputRequest,
    ) -> BuckyResult<GetBackupTaskStatusInputResponse> {
        let status = self.backup_manager.get_task_status(&req.request.id)?;

        Ok(GetBackupTaskStatusInputResponse {
            id: req.request.id,
            status,
        })
    }
}
