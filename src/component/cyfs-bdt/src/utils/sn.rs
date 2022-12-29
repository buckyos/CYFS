use std::{
    time::Duration
};
use async_std::{
    task,
    future
};
use crate::{
    stack::*, 
    sn::client::*
};


pub fn retry_sn_list_when_offline(stack: StackGuard, clients: PingClients, interval: Duration) {
    if let Some(status) = clients.status() {
        if SnStatus::Offline == status {
            if let Some(clients) = stack.sn_client().reset() {
                task::spawn(async move {
                    if let Ok(status) = clients.wait_online().await {
                        if let SnStatus::Online = status {
                            let _ = clients.wait_offline().await;
                        } 
                        let _ = future::timeout(interval, future::pending::<()>()).await;
                        retry_sn_list_when_offline(stack, clients, interval);
                    }
                });
            }
        }
    }
}
