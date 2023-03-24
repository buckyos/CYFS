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

impl UniBackupParams {
    pub fn dir(&self) -> std::borrow::Cow<PathBuf> {
        match &self.target_file.dir {
            Some(dir) => std::borrow::Cow::Borrowed(dir),
            None => {
                let dir = if self.isolate.is_empty() {
                    cyfs_util::get_cyfs_root_path_ref().join(format!("data/backup/{}", self.id))
                } else {
                    cyfs_util::get_cyfs_root_path_ref()
                        .join(format!("data/backup/{}/{}", self.isolate, self.id))
                };

                std::borrow::Cow::Owned(dir)
            }
        }
    }
}
