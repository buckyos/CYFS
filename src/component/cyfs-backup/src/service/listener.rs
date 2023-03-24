use super::{handler::BackupRequestHandler, request::BackupInputHttpRequest};
use cyfs_lib::*;

enum BackupRequestType {
    StartBackupTask,
    GetBackupTaskStatus,
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
        }
    }

    pub fn register_server(
        protocol: &RequestProtocol,
        handler: &BackupRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
        let path = format!("/backup/backup");

        server.at(&path).post(Self::new(
            protocol.clone(),
            BackupRequestType::StartBackupTask,
            handler.clone(),
        ));

        server.at(&path).get(Self::new(
            protocol.clone(),
            BackupRequestType::GetBackupTaskStatus,
            handler.clone(),
        ));
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
