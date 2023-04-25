use crate::archive_download::*;
use crate::backup::RestoreManagerRef;
use cyfs_backup_lib::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RemoteRestoreTaskPhase {
    Init,
    Download,
    Unpack,
    Restore,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteRestoreStatus {
    pub phase: RemoteRestoreTaskPhase,
    pub result: Option<BuckyResult<()>>,

    pub download_progress: Option<ArchiveProgress>,
    pub unpack_progress: Option<ArchiveProgress>,
    pub restore_status: Option<RestoreStatus>,
}

impl Default for RemoteRestoreStatus {
    fn default() -> Self {
        Self {
            phase: RemoteRestoreTaskPhase::Init,
            result: None,

            download_progress: None,
            unpack_progress: None,
            restore_status: None,
        }
    }
}


pub struct RemoteRestoreStatusManagerInner {
    pub phase: RemoteRestoreTaskPhase,
    pub result: Option<BuckyResult<()>>,

    pub download_progress: Option<ArchiveProgressHolder>,
    pub unpack_progress: Option<ArchiveProgressHolder>,
    pub restore_status: Option<(String, RestoreManagerRef)>,
}

impl RemoteRestoreStatusManagerInner {
    pub fn new() -> Self {
        Self {
            phase: RemoteRestoreTaskPhase::Init,
            result: None,
            download_progress: None,
            unpack_progress: None,
            restore_status: None,
        }
    }

    pub fn begin_download(&mut self, progress: ArchiveProgressHolder) {
        assert_eq!(self.phase, RemoteRestoreTaskPhase::Init);
        assert!(self.download_progress.is_none());

        self.phase = RemoteRestoreTaskPhase::Download;
        self.download_progress = Some(progress);
    }

    pub fn begin_unpack(&mut self, progress: ArchiveProgressHolder) {
        assert!(self.unpack_progress.is_none());

        self.phase = RemoteRestoreTaskPhase::Unpack;
        self.unpack_progress = Some(progress);
    }

    pub fn begin_restore(&mut self, task_id: &str, restore_manager: RestoreManagerRef) {
        assert!(self.restore_status.is_none());

        self.phase = RemoteRestoreTaskPhase::Restore;
        self.restore_status = Some((task_id.to_owned(), restore_manager));
    }

    pub fn complete(&mut self, result: BuckyResult<()>) {
        self.phase = RemoteRestoreTaskPhase::Complete;
        self.result = Some(result);
    }

    pub fn status(&self) -> RemoteRestoreStatus {
        let mut status = RemoteRestoreStatus::default();
        status.phase = self.phase;
        
        match self.phase {
            RemoteRestoreTaskPhase::Init => {}
            RemoteRestoreTaskPhase::Download => {
                let progress = self.download_progress.as_ref().unwrap().get_progress();
                status.download_progress = Some(progress);
            }
            RemoteRestoreTaskPhase::Unpack => {
                let progress = self.unpack_progress.as_ref().unwrap().get_progress();
                status.unpack_progress = Some(progress);
            }
            RemoteRestoreTaskPhase::Restore => {
                let (id, manager) = self.restore_status.as_ref().unwrap();
                let restore_status = manager.get_task_status(id).unwrap();
                status.restore_status = Some(restore_status);
            }
            RemoteRestoreTaskPhase::Complete => {
                status.result = self.result.clone();
            }
        }

        status
    }
}

#[derive(Clone)]
pub struct RemoteRestoreStatusManager(Arc<Mutex<RemoteRestoreStatusManagerInner>>);

impl RemoteRestoreStatusManager {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(RemoteRestoreStatusManagerInner::new())))
    }

    pub fn begin_download(&self, progress: ArchiveProgressHolder) {
        self.0.lock().unwrap().begin_download(progress)
    }

    pub fn begin_unpack(&self, progress: ArchiveProgressHolder) {
        self.0.lock().unwrap().begin_unpack(progress)
    }

    pub fn begin_restore(&self, task_id: &str, restore_manager: RestoreManagerRef) {
        self.0.lock().unwrap().begin_restore(task_id, restore_manager)
    }

    pub fn complete(&self, result: BuckyResult<()>) {
        self.0.lock().unwrap().complete(result)
    }

    pub fn status(&self) -> RemoteRestoreStatus {
        self.0.lock().unwrap().status()
    }
}
