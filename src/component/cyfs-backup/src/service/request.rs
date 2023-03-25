use cyfs_backup_lib::*;
use cyfs_lib::*;

pub(crate) struct BackupInputHttpRequest<State> {
    pub request: tide::Request<State>,

    pub source: RequestSourceInfo,
}

// backup service relate requests
pub struct StartBackupTaskInputRequest {
    pub source: RequestSourceInfo,

    pub request: StartBackupTaskOutputRequest,
}

pub type StartBackupTaskInputResponse = StartBackupTaskOutputResponse;


pub struct GetBackupTaskStatusInputRequest {
    pub source: RequestSourceInfo,

    pub request: GetBackupTaskStatusOutputRequest,
}

pub type GetBackupTaskStatusInputResponse = GetBackupTaskStatusOutputResponse;


// restore service relate requests
pub struct StartRestoreTaskInputRequest {
    pub source: RequestSourceInfo,

    pub request: StartRestoreTaskOutputRequest,
}

pub type StartRestoreTaskInputResponse = StartRestoreTaskOutputResponse;


pub struct GetRestoreTaskStatusInputRequest {
    pub source: RequestSourceInfo,

    pub request: GetRestoreTaskStatusOutputRequest,
}

pub type GetRestoreTaskStatusInputResponse = GetRestoreTaskStatusOutputResponse;