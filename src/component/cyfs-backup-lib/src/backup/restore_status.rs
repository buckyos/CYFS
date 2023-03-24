use super::backup_status::BackupStatInfo;
use crate::{archive::*, meta::*};
use cyfs_base::*;


#[derive(Clone, Copy, Debug)]
pub enum RestoreTaskPhase {
    Init,
    LoadAndVerify,
    RestoreKeyData,
    RestoreObject,
    RestoreChunk,
    Complete,
}

impl Default for RestoreTaskPhase {
    fn default() -> Self {
        Self::Init
    }
}

pub type RestoreStatInfo = BackupStatInfo;

#[derive(Clone, Debug)]
pub struct RestoreResult {
    pub index: ObjectArchiveIndex,
    pub uni_meta: Option<ObjectArchiveMetaForUniBackup>,
}

#[derive(Clone, Debug, Default)]
pub struct RestoreStatus {
    pub phase: RestoreTaskPhase,
    pub phase_last_update_time: u64,

    pub stat: RestoreStatInfo,
    pub complete: RestoreStatInfo,

    pub result: Option<BuckyResult<RestoreResult>>,
}
