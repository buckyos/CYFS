use crate::stack::SNMode;
use cyfs_base::*;
use cyfs_bdt::{retry_sn_list_when_offline, sn::client::PingClients, SnStatus, StackGuard};

use once_cell::sync::OnceCell;
use std::sync::Arc;

#[derive(Clone)]
pub struct BdtStackSNHelper {
    bdt_stack: Arc<OnceCell<StackGuard>>,
    sn_mode: SNMode,
}

impl BdtStackSNHelper {
    pub fn new(sn_mode: SNMode) -> Self {
        Self {
            bdt_stack: Arc::new(OnceCell::new()),
            sn_mode,
        }
    }

    pub fn bind_bdt_stack(&self, bdt_stack: StackGuard) {
        if let Err(_) = self.bdt_stack.set(bdt_stack) {
            unreachable!();
        }
    }

    pub async fn on_sn_list_changed(&self, config_sn_list: Vec<Device>) {
        // notify bdt stack
        if let Some(bdt_stack) = self.bdt_stack.get() {
            let sn_list = match self.sn_mode {
                SNMode::Normal => Some(config_sn_list.clone()),
                SNMode::None => {
                    warn!("sn_mode is none, now will clear the sn_list!");
                    None
                }
            };

            bdt_stack.reset_known_sn(config_sn_list);

            if let Some(sn_list) = sn_list {
                let ping_clients = bdt_stack.reset_sn_list(sn_list);
                Self::wait_sn_online(&bdt_stack, ping_clients).await;
            }
        }
    }

    pub async fn wait_sn_online(bdt_stack: &StackGuard, ping_clients: PingClients) {
        // Waiting for SN to go online
        info!(
            "now will wait for sn online {}......",
            bdt_stack.local_device_id()
        );
        let begin = std::time::Instant::now();
        match ping_clients.wait_online().await {
            Err(e) => {
                error!(
                    "bdt stack wait sn online failed! {}, during={}ms, {}",
                    bdt_stack.local_device_id(),
                    begin.elapsed().as_millis(),
                    e
                );
            }
            Ok(status) => match status {
                SnStatus::Online => {
                    info!(
                        "bdt stack sn online success! {}, during={}ms",
                        bdt_stack.local_device_id(),
                        begin.elapsed().as_millis(),
                    );
                    let bdt_stack = bdt_stack.clone();
                    async_std::task::spawn(async move {
                        let _ = ping_clients.wait_offline().await;
                        retry_sn_list_when_offline(
                            bdt_stack.clone(),
                            ping_clients,
                            std::time::Duration::from_secs(30),
                        );
                    });
                }
                SnStatus::Offline => {
                    error!(
                        "bdt stack wait sn online failed! {}, during={}ms, offline",
                        bdt_stack.local_device_id(),
                        begin.elapsed().as_millis(),
                    );
                    retry_sn_list_when_offline(
                        bdt_stack.clone(),
                        ping_clients,
                        std::time::Duration::from_secs(30),
                    );
                }
            },
        }
    }
}
