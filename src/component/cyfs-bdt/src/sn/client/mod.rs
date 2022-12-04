mod ping;
mod call;

pub use ping::*;
use crate::sn::client::call::CallManager;
use crate::interface::udp::OnUdpPackageBox;
use crate::interface::udp;
use cyfs_base::*;
use crate::protocol::{*, v0::*};
use std::future::Future;
use crate::stack::{Stack, WeakStack};
use crate::{SnServiceReceipt, SnServiceGrade};

pub struct ClientManager {
    ping: PingManager,
    call: CallManager,
}

impl ClientManager {
    pub fn create(stack: WeakStack) -> ClientManager {
        let strong_stack = Stack::from(&stack); 
        let config = &strong_stack.config().sn_client;
        ClientManager {
            ping: PingManager::create(stack.clone(), config),
            call: CallManager::create(stack, config)
        }
    }

    pub fn reset(&self) {
        self.ping.reset();
    }

    pub fn sn_list(&self) -> Vec<DeviceId> {
        self.ping.sn_list()
    }

    pub fn status_of(&self, sn: &DeviceId) -> Option<SnStatus> {
        self.ping.status_of(sn)
    }

    pub fn start_ping(&self) {
        self.ping.start()
    }

    pub fn stop_ping(&self) -> Result<(), BuckyError> {
        self.ping.stop()
    }

    pub fn add_sn_ping(&self, desc: &Device, is_encrypto: bool, appraiser: Option<Box<dyn ServiceAppraiser>>) {
        struct NoneSnServiceAppraiser;
        impl ServiceAppraiser for NoneSnServiceAppraiser {
            fn appraise(&self, _sn: &Device, _local_receipt: &Option<SnServiceReceipt>, _last_receipt: &Option<SnServiceReceipt>, _receipt_from_sn: &Option<SnServiceReceipt>) -> SnServiceGrade {
                SnServiceGrade::Wonderfull
            }
        }

        let appraiser = match appraiser {
            Some(ap) => ap,
            None => Box::new(NoneSnServiceAppraiser)
        };

        let _ = self.ping.add_sn(desc, is_encrypto, appraiser);
    }

    pub fn remove_sn_ping(&self, sn_peerid: &DeviceId) -> Result<(), BuckyError> {
        self.ping.remove_sn(sn_peerid)
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

    pub fn resend_ping(&self) {
        self.ping.resend_ping()
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
                            let _ = self.ping.on_ping_resp(ping_resp, &from, from_interface.clone());
                        }
                    }
                },
                PackageCmdCode::SnCalled => {
                    match pkg.as_any().downcast_ref::<SnCalled>() {
                        None => return Err(BuckyError::new(BuckyErrorCode::InvalidData, "should be SnCalled")),
                        Some(called) => {
                            let _ = self.ping.on_called(called, package_box.as_ref(), &from, from_interface.clone());
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

