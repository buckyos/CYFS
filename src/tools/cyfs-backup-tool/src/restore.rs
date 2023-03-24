use cyfs_base::*;
use cyfs_backup::{RestoreManagerRef, RestoreManager};

use std::sync::Arc;

pub struct RestoreService {
    restore_manager: RestoreManagerRef,
}

impl RestoreService {
    pub async fn new(_isolate: &str) -> BuckyResult<Self> {
        let restore_manager = RestoreManager::new();

        let ret = Self {
            restore_manager: Arc::new(restore_manager),
        };

        Ok(ret)
    }

    pub fn restore_manager(&self) -> &RestoreManagerRef {
        &self.restore_manager
    }
}