use cyfs_base::*;
use cyfs_bdt::{retry_sn_list_when_offline, SnStatus, StackGuard};

use once_cell::sync::OnceCell;
use std::sync::Arc;
use std::time::Duration;

pub struct BdtStackSNHelper {
    bdt_stack: Arc<OnceCell<StackGuard>>,
}

impl BdtStackSNHelper {
    pub fn new() -> Self {
        Self {
            bdt_stack: Arc::new(OnceCell::new()),
        }
    }
    
    pub fn bind_bdt_stack(&self, bdt_stack: StackGuard) {
        if let Err(_) = self.bdt_stack.set(bdt_stack) {
            unreachable!();
        }
    }

    pub async fn on_sn_list_changed(&self, sn_list: Vec<Device>) {
        // notify bdt stack
        if let Some(bdt_stack) = self.bdt_stack.get() {
            let ping_clients = bdt_stack.reset_sn_list(sn_list);
            match ping_clients.wait_online().await {
                Err(err) => {
                    error!("reset bdt sn list error! {}", err);
                }
                Ok(status) => match status {
                    SnStatus::Online => {
                        info!("reset bdt sn list success!");
                        let bdt_stack = bdt_stack.clone();
                        async_std::task::spawn(async move {
                            let _ = ping_clients.wait_offline().await;
                            retry_sn_list_when_offline(
                                bdt_stack.clone(),
                                ping_clients,
                                Duration::from_secs(30),
                            );
                        });
                    }
                    SnStatus::Offline => {
                        error!("reset bdt sn list error! offline");
                        retry_sn_list_when_offline(
                            bdt_stack.clone(),
                            ping_clients,
                            Duration::from_secs(30),
                        );
                    }
                },
            }
        }
    }
}
