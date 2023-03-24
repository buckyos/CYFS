use cyfs_base::*;
use crate::*;

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