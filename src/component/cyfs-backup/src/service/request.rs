use cyfs_backup_lib::*;
use cyfs_lib::*;

pub(crate) struct BackupInputHttpRequest<State> {
    pub request: tide::Request<State>,

    pub source: RequestSourceInfo,
}


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