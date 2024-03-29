use super::stack::StackComponentsHelper;
use cyfs_backup::{BackupManager, BackupManagerRef};
use cyfs_base::*;

use std::sync::Arc;

pub struct BackupService {
    backup_manager: BackupManagerRef,
}

impl BackupService {
    pub async fn new(isolate: &str) -> BuckyResult<Self> {
        let noc = StackComponentsHelper::init_noc(isolate).await?;
        let ndc = StackComponentsHelper::init_ndc(isolate)?;
        let chunk_manager = StackComponentsHelper::init_chunk_manager(isolate).await?;

        let tracker = StackComponentsHelper::init_tracker(isolate)?;
        let chunk_reader = StackComponentsHelper::create_chunk_reader(chunk_manager, &ndc, tracker);

        let backup_manager = BackupManager::new(noc, ndc, chunk_reader);

        let ret = Self { backup_manager: Arc::new(backup_manager), };

        Ok(ret)
    }

    pub fn backup_manager(&self) -> &BackupManagerRef {
        &self.backup_manager
    }
}
