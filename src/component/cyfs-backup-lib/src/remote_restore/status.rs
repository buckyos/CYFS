use crate::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};

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


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn display() {
        let e = BuckyError::new(BuckyErrorCode::NotFound, "not found error");

        let mut progress = ArchiveProgress::new();
        progress.current = Some(FileProgress {
            file: "object.0.data".to_owned(),
            total: 100000,
            completed: 100,
            result: None,
        });

        let status = RemoteRestoreStatus {
            phase: RemoteRestoreTaskPhase::Download,
            // result: Some(Ok(())),
            result: Some(Err(e)),

            download_progress: Some(progress),
            unpack_progress: Some(ArchiveProgress::new()),
            restore_status: Some(RestoreStatus::default()),
        };

        let s = serde_json::to_string_pretty(&status).unwrap();
        println!("{}", s);
    }
}
