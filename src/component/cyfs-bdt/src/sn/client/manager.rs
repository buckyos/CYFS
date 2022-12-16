use std::{
    time::Duration, 
    sync::{Arc, RwLock}, 
};

use async_std::{
    task, 
    future
};

use cyfs_base::*;
use crate::{
    types::*, 
    interface::{NetListener, udp::{self, OnUdpPackageBox}}, 
    protocol::{*, v0::*}, 
    stack::{Stack, WeakStack}
};
use super::{
    ping::{PingConfig, PingClients}, 
    call::{CallConfig, CallManager, CallSessions}
};

pub trait PingClientCalledEvent<Context=()>: Send + Sync {
    fn on_called(&self, called: &SnCalled, context: Context) -> Result<(), BuckyError>;
}


#[derive(Clone)]
pub struct Config {
    pub atomic_interval: Duration, 
    pub ping: PingConfig, 
    pub call: CallConfig,
}

struct ManagerImpl {
    stack: WeakStack, 
    gen_seq: Arc<TempSeqGenerator>, 
    ping: RwLock<PingClients>, 
    call: CallManager,
}

#[derive(Clone)]
pub struct ClientManager(Arc<ManagerImpl>);

impl ClientManager {
    pub fn create(stack: WeakStack, net_listener: NetListener, local_device: Device) -> Self {
        let strong_stack = Stack::from(&stack); 
        let config = &strong_stack.config().sn_client;
        let atomic_interval = config.atomic_interval;
        let gen_seq = Arc::new(TempSeqGenerator::new());
        let manager = Self(Arc::new(ManagerImpl {
            ping: RwLock::new(PingClients::new(stack.clone(), gen_seq.clone(), net_listener, vec![], local_device)),
            call: CallManager::create(stack.clone()), 
            gen_seq, 
            stack, 
        }));

        {
            let manager = manager.clone();
            task::spawn(async move {
                loop {
                    let now = bucky_time_now();
                    manager.ping().on_time_escape(now);
                    manager.call().on_time_escape(now);
                    let _ = future::timeout(atomic_interval, future::pending::<()>()).await;
                }
            });
        }
        manager
    }

    pub fn ping(&self) -> PingClients {
        self.0.ping.read().unwrap().clone()
    }

    pub fn reset(&self, sn_list: Vec<Device>) -> PingClients {
        let (to_start, to_close) = {
            let mut ping = self.0.ping.write().unwrap();
            let to_close = ping.clone();
            let to_start = PingClients::new(
                self.0.stack.clone(), 
                self.0.gen_seq.clone(), 
                to_close.net_listener().reset(None).unwrap(), 
                sn_list, 
                to_close.default_local()
            );
            *ping = to_start.clone();
            (to_start, to_close)
        };
        to_close.stop();
        to_start
    }

    pub fn call(&self) -> &CallManager {
        &self.0.call
    }
}

impl OnUdpPackageBox for ClientManager {
    fn on_udp_package_box(&self, package_box: udp::UdpPackageBox) -> Result<(), BuckyError> {
        let from = package_box.remote().clone();
        let from_interface = package_box.local();
        for pkg in package_box.as_ref().packages() {
            match pkg.cmd_code() {
                PackageCmdCode::SnPingResp => {
                    match pkg.as_any().downcast_ref::<SnPingResp>() {
                        None => return Err(BuckyError::new(BuckyErrorCode::InvalidData, "should be SnPingResp")),
                        Some(ping_resp) => {
                            let _ = self.ping().on_udp_ping_resp(ping_resp, &from, from_interface.clone());
                        }
                    }
                },
                PackageCmdCode::SnCalled => {
                    match pkg.as_any().downcast_ref::<SnCalled>() {
                        None => return Err(BuckyError::new(BuckyErrorCode::InvalidData, "should be SnCalled")),
                        Some(called) => {
                            let _ = self.ping().on_called(called, package_box.as_ref(), &from, from_interface.clone());
                        }
                    }
                },
                PackageCmdCode::SnCallResp => {
                    match pkg.as_any().downcast_ref::<SnCallResp>() {
                        None => return Err(BuckyError::new(BuckyErrorCode::InvalidData, "should be SnCallResp")),
                        Some(call_resp) => {
                            let _ = self.call().on_udp_call_resp(call_resp, from_interface, &from);
                        }
                    }
                },
                _ => {
                    return Err(BuckyError::new(BuckyErrorCode::InvalidData, format!("unkown package({:?})", pkg.cmd_code()).as_str()))
                }
            }
        }

        Ok(())
    }
}

