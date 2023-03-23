use crate::{archive::*, meta::*};
use cyfs_base::*;


#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Debug, Default)]
pub struct BackupStatInfo {
    pub objects: ObjectArchiveDataMeta,
    pub chunks: ObjectArchiveDataMeta,
    pub files: ObjectArchiveDataMeta,
}

#[derive(Clone, Debug)]
pub struct BackupResult {
    pub index: ObjectArchiveIndex,
    pub uni_meta: Option<ObjectArchiveMetaForUniBackup>,
}

#[derive(Clone, Debug, Default)]
pub struct BackupStatus {
    pub phase: BackupTaskPhase,
    pub phase_last_update_time: u64,

    pub stat: BackupStatInfo,
    pub complete: BackupStatInfo,

    pub result: Option<BuckyResult<BackupResult>>,
}