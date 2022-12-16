use log::*;
use std::{
    sync::{Arc, RwLock,}, 
    time::{Duration}, 
    collections::LinkedList
};
use async_std::{
    task,
    future
};

use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::{NetListener, UpdateOuterResult, udp::{Interface, PackageBoxEncodeContext}}, 
    history::keystore, 
    stack::{WeakStack, Stack} 
};
use super::{ 
    manager::{PingSession, PingSessionResp},
};

#[derive(Clone)]
pub struct Config {
    pub timeout: Duration,

    pub ping_interval: Duration,
}


struct SessionImpl {
    stack: WeakStack,
    config: Config, 
    with_device: bool, 
    local: Interface, 
    local_device: Device,
    gen_seq: Arc<TempSeqGenerator>, 
    sn_desc: DeviceDesc,
    sn_endpoints: Vec<Endpoint>,  
    state: RwLock<SessionState>
}

#[derive(Clone)]
struct UdpPingSession(Arc<SessionImpl>);

enum SessionState {
    Requesting {
        first_sent_time: Timestamp, 
        last_sent_time: Timestamp, 
        first_sent_seq: TempSeq, 
        last_sent_seq: TempSeq, 
        waiter: StateWaiter
    }, 
    Responsed {
        first_resp_time: Timestamp, 
        resp: PingSessionResp
    }, 
    Timeout, 
    Canceled
}

pub struct UdpSesssionParams {
    config: Config, 
    local: Interface,
    local_device: Device, 
    with_device: bool, 
    sn_id: DeviceId, 
    sn_desc: DeviceDesc,
    sn_endpoints: Vec<Endpoint>,  
}

impl UdpPingSession {
    pub fn new(stack: WeakStack,  gen_seq: Arc<TempSeqGenerator>, params: UdpSesssionParams) -> Self {
        let seq = gen_seq.generate();
        let now = bucky_time_now();
        let session = Self(Arc::new(SessionImpl {
            stack, 
            gen_seq, 
            config: params.config, 
            local: params.local, 
            local_device: params.local_device, 
            with_device: params.with_device, 
            sn_id: params.sn_desc.device_id(), 
            sn_desc: params.sn_desc, 
            sn_endpoints: params.sn_endpoints, 
            state: RwLock::new(SessionState::Requesting {
                    last_sent_time: now,
                    first_sent_time: now,  
                    first_sent_seq: seq, 
                    last_sent_seq: seq, 
                    waiter: StateWaiter::new()
                })
        }));

        {
            let session = session.clone();
            task::spawn(async move {
                let _ = session.send_ping(seq).await;
            })
        }
        
        session
    }


    async fn send_ping(&self, seq: TempSeq) -> BuckyResult<()> {
        let stack = Stack::from(&self.0.stack);
        
        let ping_pkg = SnPing {
            protocol_version: 0, 
            stack_version: 0, 
            seq,
            from_peer_id: Some(stack.local_device_id().clone()),
            sn_peer_id: self.sn().clone(),
            peer_info: if self.0.with_device { Some(self.0.local_device.clone()) } else { None }, 
            send_time: bucky_time_now(),
            contract_id: None, 
            receipt: None
        };

        let key_stub = stack.keystore().create_key(&self.0.sn_desc, true);

        let mut pkg_box = PackageBox::encrypt_box(
            self.sn().clone(), 
            key_stub.key.clone());

        if let keystore::EncryptedKey::Unconfirmed(key_encrypted) = key_stub.encrypted {
            let mut exchg = Exchange::from((&ping_pkg, self.0.local_device.clone(), key_encrypted, key_stub.key.mix_key));
            let _ = exchg.sign(stack.keystore().signer()).await;
            pkg_box.push(exchg);
        }
        pkg_box.push(ping_pkg);


        struct SendPingIter {
            interface: Interface, 
            endpoints: LinkedList<Endpoint>
        }

        impl Iterator for SendPingIter {
            type Item = (Interface, Endpoint);

            fn next(&mut self) -> Option<Self::Item> {
                self.endpoints.pop_front().map(|ep| (self.interface.clone(), ep))
            }
        }


        
        info!("{} send sn ping, seq={:?}", self, seq);
        let mut iter = SendPingIter {
            interface: self.0.interface.clone(), 
            endpoints: {
                let mut endpoints = LinkedList::new();
                for endpoint in &self.0.endpoints {
                    endpoints.push_back(*endpoint);
                }
                endpoints
            }
        };
        let mut context = PackageBoxEncodeContext::default();
        let _ = Interface::send_box_mult(
            &mut context, 
            &pkg_box, 
            iter,
            |from, to, result| {
                log::debug!("{} ping seq:{:?} from {} to {}/{}, result: {:?}", self, seq, from.local(), self.sn(), to, result);
                true
            })?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl PingSession for UdpPingSession {
    fn sn(&self) -> &DeviceId {
        &self.0.sn_desc.device_id()
    }

    fn from(&self) -> Endpoint {
        self.0.local.local()
    }

    fn clone_as_ping_session(&self) -> Box<dyn PingSession> {
        Box::new(self.clone())
    }

    fn on_time_escape(&self, now: Timestamp) {
        enum NextStep {
            None,
            SendPing(TempSeq), 
            Timeout(StateWaiter), 
        }
        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                SessionState::Requesting {
                    waiter,  
                    first_sent_time, 
                    last_sent_time, 
                    first_sent_seq, 
                    last_sent_seq  
                } => {
                    if now > *first_sent_time && Duration::from_micros(now - *first_sent_time) > self.0.config.resend_timeout {
                        let waiter = waiter.transfer();
                        *state = SessionState::Timeout;
                        NextStep::Timeout(waiter)
                    } else if now > *last_sent_time && Duration::from_micros(now - *last_sent_time) > self.0.config.resend_interval {
                        let seq = self.0.gen_seq.generate();
                        *last_sent_seq = seq;
                        *last_sent_time = now;
                        NextStep::SendPing(seq)
                    } else {
                        NextStep::None
                    }
                }, 
                _ => NextStep::None
            }
        };

        match next {
            NextStep::SendPing(seq) => {
                let session = self.clone();
                task::spawn(async move {
                    let _ = session.send_ping(seq).await;
                });
            },
            NextStep::Timeout(waiter) => {
                waiter.wake();
            },
            _ => {}
        };
        

    }

    async fn wait(&self) -> BuckyResult<PingSessionResp> {
        let (waiter, result) = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                SessionState::Requesting {waiter, ..} => (Some(waiter.new_waiter()), Err(BuckyError::new(BuckyErrorCode::Pending, ""))),
                SessionState::Canceled => (None, Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"))), 
                SessionState::Timeout => (None, Err(BuckyError::new(BuckyErrorCode::Timeout, "sn server no response"))), 
                SessionState::Responsed { resp, .. } => {None, Ok(resp.clone())}
            }
        };
        
        if let Some(waiter) = waiter {
            StateWaiter::wait(waiter, || {
                let state = self.0.state.read().unwrap();
                match &*state {
                    SessionState::Requesting {..} => unreachable!(),
                    SessionState::Canceled => Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled")), 
                    SessionState::Timeout => Err(BuckyError::new(BuckyErrorCode::Timeout, "sn server no response")), 
                    SessionState::Responsed { resp, .. } => Ok(resp.clone())
                }
            });
        } else {
            result
        }
    }

    fn stop(&self) {
        let waiter = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                SessionState::Requesting {waiter, ..} => {
                    let waiter = waiter.transfer();
                    *state = SessionState::Canceled;
                    Some(waiter)
                },
                _ => None
            }
        };
       
        if let Some(waiter) = waiter {
            waiter.wake();
        }
    }

    fn on_udp_ping_resp(&self, resp: &SnPingResp, from: &Endpoint) -> BuckyResult<()> {
        let now = bucky_time_now();
        
        let waiter = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                SessionState::Requesting {
                    first_sent_seq, 
                    last_sent_seq, 
                    waiter, 
                    ..
                } => {
                    if *first_sent_seq <= resp.seq && *last_sent_seq >= resp.seq {
                        let resp = PingSessionResp {
                            from: *from, 
                            err: BuckyErrorCode::from(resp.result as u16), 
                            endpoints: resp.end_point_array.clone()
                        };
                        let waiter = waiter.transfer();
                        *state = SessionState::Responsed { 
                            first_resp_time: now, 
                            resp
                        };
                        Some(waiter)
                    } else {
                        None
                    }
                }, 
                _ => None
            }
        };

        if let Some(waiter) = waiter {
            waiter.wake();
        }
        Ok(())
    }
}




