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

impl RemoteRestoreParams {
    pub fn new(id: impl Into<String>, remote_archive: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            cyfs_root: None,
            isolate: None,
            password: None,
            remote_archive: remote_archive.into(),
        }
    }
}