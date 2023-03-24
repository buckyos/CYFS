use crate::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupOutputRequestCommon {
    pub target: Option<ObjectId>,
    pub dec_id: Option<ObjectId>,
    pub flags: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartBackupTaskOutputRequest {
    pub common: BackupOutputRequestCommon,

    pub params: UniBackupParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartBackupTaskOutputResponse {
    pub result: BuckyResult<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBackupTaskStatusOutputRequest {
    pub common: BackupOutputRequestCommon,

    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBackupTaskStatusOutputResponse {
    pub id: String,
    pub status: BackupStatus,
}

// restore relate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRestoreTaskOutputRequest {
    pub common: BackupOutputRequestCommon,

    pub params: UniRestoreParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRestoreTaskOutputResponse {
    pub result: BuckyResult<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRestoreTaskStatusOutputRequest {
    pub common: BackupOutputRequestCommon,

    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRestoreTaskStatusOutputResponse {
    pub id: String,
    pub status: RestoreStatus,
}
