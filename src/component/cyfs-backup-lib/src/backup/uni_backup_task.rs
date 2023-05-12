use crate::crypto::*;
use crate::object_pack::*;

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize,)]
pub struct LocalFileBackupParam {
    // Backup target_file storage directory
    pub dir: Option<PathBuf>,

    // Inner data folder name in archive, default is "data"
    pub data_folder: Option<String>,

    pub format: ObjectPackFormat,

    pub file_max_size: u64,
}

impl Default for LocalFileBackupParam {
    fn default() -> Self {
        Self {
            dir: None,
            data_folder: Some("data".to_owned()),
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

    // Key data filters in glob format
    pub key_data_filters: Vec<String>,
}