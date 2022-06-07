use super::http_server::{DefaultHttpServer, HttpDefaultHandler, RawHttpServer};
use super::{ObjectHttpBdtListener, ObjectListener, SyncHttpListener};
use crate::sync::*;
use cyfs_base::BuckyResult;
use cyfs_lib::NONProtocol;
use cyfs_bdt::StackGuard;

use std::sync::Arc;

pub struct SyncListenerManagerParams {
    pub bdt_stack: StackGuard,

    // bdt协议栈监听的vport列表
    pub bdt_listeners: Vec<u16>,
}

pub(crate) struct SyncListenerManager {
    listeners: Vec<Box<dyn ObjectListener>>,
}

impl SyncListenerManager {
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    pub fn init(
        &mut self,
        params: SyncListenerManagerParams,
        sync_server: Option<&Arc<ZoneSyncServer>>,
        sync_client: Option<&Arc<DeviceSyncClient>>,
    ) {
        assert!(self.listeners.is_empty());

        let default_handler = HttpDefaultHandler::default();

        for vport in params.bdt_listeners {
            info!("new http-bdt sync bdt listener: vport={}", vport);
            let server = SyncHttpListener::new(NONProtocol::HttpBdt, sync_server, sync_client);
            let handler = RawHttpServer::new(server.into_server()).into();
            let http_server = DefaultHttpServer::new(handler, None, default_handler.clone());

            let bdt_listener =
                ObjectHttpBdtListener::new(params.bdt_stack.clone(), vport, http_server.into());
            let bdt_listener = Box::new(bdt_listener) as Box<dyn ObjectListener>;
            self.listeners.push(bdt_listener);
        }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        for listener in &self.listeners {
            if let Err(e) = listener.start().await {
                error!(
                    "start sync listener error: addr={}, {}",
                    listener.get_addr(),
                    e
                );
                return Err(e);
            }
        }

        Ok(())
    }
}
