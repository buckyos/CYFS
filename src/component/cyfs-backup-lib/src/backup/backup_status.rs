use crate::{archive::*, meta::*};
use cyfs_base::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum BackupTaskPhase {
    Init,
    Stat,
    Backup,
    Complete,
}

impl Default for BackupTaskPhase {
    fn default() -> Self {
        Self::Init
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BackupStatInfo {
    pub objects: ObjectArchiveDataMeta,
    pub chunks: ObjectArchiveDataMeta,
    pub files: ObjectArchiveDataMeta,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupResult {
    pub index: ObjectArchiveIndex,
    pub uni_meta: Option<ObjectArchiveMetaForUniBackup>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize,)]
pub struct BackupStatus {
    pub phase: BackupTaskPhase,
    pub phase_last_update_time: u64,

    pub stat: BackupStatInfo,
    pub complete: BackupStatInfo,

    pub result: Option<BuckyResult<BackupResult>>,
}