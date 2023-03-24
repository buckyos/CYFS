use cyfs_backup::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;

pub struct BackupInterface {
    interface: HttpInterface,
}

impl BackupInterface {
    pub fn new(
        mode: BackupHttpServerMode,
        backup_manager: Option<BackupManagerRef>,
        restore_manager: Option<RestoreManagerRef>,
        host: HttpInterfaceHost,
    ) -> Self {
        let mut server = HttpServer::new_server();
        Self::register(mode, backup_manager, restore_manager, &mut server);

        let interface = HttpInterface::new(host, OOD_BACKUP_TOOL_SERVICE_PORT, server);

        let ret = Self { interface };

        ret
    }

    fn register(
        mode: BackupHttpServerMode,
        backup_manager: Option<BackupManagerRef>,
        restore_manager: Option<RestoreManagerRef>,
        server: &mut tide::Server<()>,
    ) {
        let service = cyfs_backup::BackupService::new_direct(backup_manager, restore_manager)
            .into_processor();

        let handler = cyfs_backup::BackupRequestHandler::new(service);

        cyfs_backup::BackupRequestHandlerEndpoint::register_server(
            mode,
            &RequestProtocol::HttpLocal,
            &handler,
            server,
        )
    }

    pub async fn start(&self) -> BuckyResult<()> {
        self.interface.start().await
    }
}
