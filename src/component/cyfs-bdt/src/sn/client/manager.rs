use std::{
    time::Duration, 
    future::Future,
    sync::{Arc, RwLock}
};

use cyfs_base::*;
use crate::{
    types::*, 
    interface::{NetListener, udp::{self, OnUdpPackageBox}}, 
    protocol::{*, v0::*}, 
    stack::{Stack, WeakStack}
};
use super::{
    ping::{self, PingClients}, 
    call::*
};

pub trait PingClientCalledEvent<Context=()>: Send + Sync {
    fn on_called(&self, called: &SnCalled, context: Context) -> Result<(), BuckyError>;
}


#[derive(Clone)]
pub struct Config {
    pub ping: ping::Config, 
    pub call_interval: Duration,
    pub call_timeout: Duration,
}

pub struct ClientManager {
    stack: WeakStack, 
    gen_seq: Arc<TempSeqGenerator>, 
    ping: RwLock<PingClients>, 
    pub(super) call: CallManager,
}

impl ClientManager {
    pub fn create(stack: WeakStack, net_listener: NetListener, local_device: Device) -> ClientManager {
        let strong_stack = Stack::from(&stack); 
        let config = &strong_stack.config().sn_client;
        let gen_seq = Arc::new(TempSeqGenerator::new());
        ClientManager {
            ping: RwLock::new(PingClients::new(stack.clone(), gen_seq.clone(), net_listener, vec![], local_device)),
            call: CallManager::create(stack.clone(), config), 
            gen_seq, 
            stack, 
        }
    }

    pub fn ping(&self) -> PingClients {
        self.ping.read().unwrap().clone()
    }

    pub fn reset(&self, sn_list: Vec<Device>) -> PingClients {
        let (to_start, to_close) = {
            let mut ping = self.ping.write().unwrap();
            let to_close = ping.clone();
            let to_start = PingClients::new(
                self.stack.clone(), 
                self.gen_seq.clone(), 
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

    pub fn call(&self,
                reverse_endpoints: &[Endpoint], 
                remote_peerid: &DeviceId,
                sn: &Device,
                is_always_call: bool,
                is_encrypto: bool,
                with_local: bool,
                payload_generater: impl Fn(&SnCall) -> Vec<u8>
    ) -> impl Future<Output = Result<Device, BuckyError>> {
        self.call.call(reverse_endpoints, remote_peerid, sn, is_always_call, is_encrypto, with_local, payload_generater)
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
                            let _ = self.call.on_call_resp(call_resp, &from);
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

