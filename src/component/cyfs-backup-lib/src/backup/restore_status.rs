use super::backup_status::BackupStatInfo;
use crate::{archive::*, meta::*};
use cyfs_base::*;

use serde::{Deserialize, Serialize};


#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum RestoreTaskPhase {
    Init,
    LoadAndVerify,
    RestoreObject,
    RestoreChunk,
    RestoreKeyData,
    Complete,
}

impl Default for RestoreTaskPhase {
    fn default() -> Self {
        Self::Init
    }
}

pub type RestoreStatInfo = BackupStatInfo;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestoreResult {
    pub index: ObjectArchiveIndex,
    pub uni_meta: Option<ObjectArchiveMetaForUniBackup>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RestoreStatus {
    pub phase: RestoreTaskPhase,
    pub phase_last_update_time: u64,

    pub stat: RestoreStatInfo,
    pub complete: RestoreStatInfo,

    pub result: Option<BuckyResult<RestoreResult>>,
}
