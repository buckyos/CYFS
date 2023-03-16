use cyfs_base::*;
use cyfs_backup_lib::RestoreManager;


pub struct RestoreService {
    restore_manager: RestoreManager,
}

impl RestoreService {
    pub async fn new(_isolate: &str) -> BuckyResult<Self> {
        let restore_manager = RestoreManager::new();

        let ret = Self {
            restore_manager,
        };

        Ok(ret)
    }

    pub fn restore_manager(&self) -> &RestoreManager {
        &self.restore_manager
    }
}