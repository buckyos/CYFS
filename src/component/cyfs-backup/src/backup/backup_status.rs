use cyfs_backup_lib::*;
use cyfs_base::*;

use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct BackupStatusManager {
    status: Arc<Mutex<BackupStatus>>,
}

impl BackupStatusManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(BackupStatus::default())),
        }
    }

    pub fn status(&self) -> BackupStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn init_stat(&self, stat: BackupStatInfo) {
        let mut status = self.status.lock().unwrap();
        status.stat = stat;
    }

    pub fn update_phase(&self, phase: BackupTaskPhase) -> BackupTaskPhase {
        let mut status = self.status.lock().unwrap();
        let cur = status.phase;
        status.phase = phase;
        status.phase_last_update_time = bucky_time_now();

        cur
    }

    pub fn on_file(&self) {
        let mut status = self.status.lock().unwrap();
        status.complete.files.count += 1;
    }

    pub fn on_object(&self) {
        let mut status = self.status.lock().unwrap();
        status.complete.objects.count += 1;
    }

    pub fn on_chunk(&self) {
        let mut status = self.status.lock().unwrap();
        status.complete.chunks.count += 1;
    }

    pub fn on_complete(&self, ret: BuckyResult<BackupResult>) {
        let mut status = self.status.lock().unwrap();
        status.result = Some(ret);
    }
}
