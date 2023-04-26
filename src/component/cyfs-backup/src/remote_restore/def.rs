use cyfs_backup_lib::*;

use serde::{Deserialize, Serialize};


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteRestoreParams {
    // TaskId, should be valid segment string of path
    pub id: String,

    // Restore related params
    pub cyfs_root: Option<String>,
    pub isolate: Option<String>,
    pub password: Option<ProtectedPassword>,

    // Remote archive info
    pub remote_archive: String,
}