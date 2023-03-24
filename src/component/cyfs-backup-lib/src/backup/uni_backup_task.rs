use crate::crypto::*;
use crate::object_pack::*;

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize,)]
pub struct LocalFileBackupParam {
    // Backup target_file storage directory
    pub dir: Option<PathBuf>,

    pub format: ObjectPackFormat,

    pub file_max_size: u64,
}

impl Default for LocalFileBackupParam {
    fn default() -> Self {
        Self {
            dir: None,
            format: ObjectPackFormat::Zip,
            file_max_size: 1024 * 1024 * 512,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize,)]
pub struct UniBackupParams {
    pub id: String,
    pub isolate: String,
    pub password: Option<ProtectedPassword>,

    pub target_file: LocalFileBackupParam,
}