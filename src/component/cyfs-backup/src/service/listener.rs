use super::{handler::BackupRequestHandler, request::BackupInputHttpRequest};
use cyfs_lib::*;

enum BackupRequestType {
    StartBackupTask,
    GetBackupTaskStatus,

    StartRestoreTask,
    GetRestoreTaskStatus,
}

pub struct BackupRequestHandlerEndpoint {
    protocol: RequestProtocol,
    req_type: BackupRequestType,
    handler: BackupRequestHandler,
}

impl BackupRequestHandlerEndpoint {
    fn new(
        protocol: RequestProtocol,
        req_type: BackupRequestType,
        handler: BackupRequestHandler,
    ) -> Self {
        Self {
            protocol,
            req_type,
            handler,
        }
    }

    async fn process_request<State: Send>(&self, request: tide::Request<State>) -> tide::Response {
        let request = BackupInputHttpRequest {
            source: RequestSourceInfo::new_local_system(),
            request,
        };

        match self.req_type {
            BackupRequestType::StartBackupTask => {
                self.handler
                    .process_start_backup_task_request(request)
                    .await
            }
            BackupRequestType::GetBackupTaskStatus => {
                self.handler
                    .process_get_backup_task_status_request(request)
                    .await
            }

            BackupRequestType::StartRestoreTask => {
                self.handler
                    .process_start_restore_task_request(request)
                    .await
            }
            BackupRequestType::GetRestoreTaskStatus => {
                self.handler
                    .process_get_restore_task_status_request(request)
                    .await
            }
        }
    }

    pub fn register_server(
        mode: BackupHttpServerMode,
        protocol: &RequestProtocol,
        handler: &BackupRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
        if mode == BackupHttpServerMode::Full {
            server.at("/backup/backup").post(Self::new(
                protocol.clone(),
                BackupRequestType::StartBackupTask,
                handler.clone(),
            ));
        }

        server.at("/backup/backup/status").post(Self::new(
            protocol.clone(),
            BackupRequestType::GetBackupTaskStatus,
            handler.clone(),
        ));

        if *protocol == RequestProtocol::HttpLocal {
            server.at("/backup/backup/status").get(Self::new(
                protocol.clone(),
                BackupRequestType::GetBackupTaskStatus,
                handler.clone(),
            ));
        }

        if mode == BackupHttpServerMode::Full {
            server.at("/backup/restore").post(Self::new(
                protocol.clone(),
                BackupRequestType::StartRestoreTask,
                handler.clone(),
            ));
        }

        server.at("/backup/restore/status").post(Self::new(
            protocol.clone(),
            BackupRequestType::GetRestoreTaskStatus,
            handler.clone(),
        ));

        if *protocol == RequestProtocol::HttpLocal {
            server.at("/backup/restore/status").get(Self::new(
                protocol.clone(),
                BackupRequestType::GetRestoreTaskStatus,
                handler.clone(),
            ));
        }
    }
}

#[async_trait::async_trait]
impl<State> tide::Endpoint<State> for BackupRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]

pub enum BackupHttpServerMode {
    Full,
    GetStatusOnly,
}

impl Default for BackupHttpServerMode {
    fn default() -> Self {
        Self::Full
    }
}
