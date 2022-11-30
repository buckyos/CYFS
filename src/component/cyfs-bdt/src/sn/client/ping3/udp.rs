use std::{
    sync::{Arc, RwLock,}, 
    collections::{BTreeMap},
    convert::TryFrom, 
    time::{Duration}
};
use async_std::{
    task
};

use cyfs_base::*;
use cyfs_debug::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::{NetListener, UpdateOuterResult, udp::{Interface, PackageBoxEncodeContext}}, 
    history::keystore, 
    stack::{WeakStack, Stack} 
};
use super::super::Config;
use super::{ 
    manager::*,
};


#[derive(Debug, Clone)]
enum ClientStatus {
    Running {
        last_ping_time: Timestamp,
        last_update_seq: Option<TempSeq>,
    },
    Stopped
}

struct ClientState {
    sessions: Vec<Box<dyn Session>>,
    active_session: Option<Session>,
   
    client_status: ClientStatus,
    sn_status: SnStatus
}

struct ClientInner {
    stack: WeakStack, 
    config: Config, 
    sn_id: DeviceId,
    sn: Device,
    seq_genarator: TempSeqGenerator,

    state: RwLock<ClientState>    
}

struct SendPingOptions {
    seq: TempSeq, 
    with_device: bool
}


#[derive(Clone)]
struct UdpClient(Arc<ClientInner>);

impl UdpClient {
    fn new(stack: WeakStack, sn: Device) -> Self {
        let sn_id = sn.desc().device_id();

        let strong_stack = Stack::from(&stack);

        let mut sessions = Vec::default();
        let net_listener = strong_stack.net_manager().listener().clone();

        for udp in net_listener.udp() {
            let session = UdpSession::new(udp.clone(), &sn);
            sessions.push(session);
        }

        let seq_genarator = TempSeqGenerator::new();
        let next_seq = seq_genarator.generate();
        let client = Self(Arc::new(ClientInner {
            stack, 
            sn_id,
            sn,
            seq_genarator, 
            state: RwLock::new(ClientState {
                sessions, 
                active_session: None, 
                sn_status: SnStatus::Connecting, 
                client_status: ClientStatus::Running {
                    last_ping_time: 0, 
                    last_update_seq: Some(next_seq)
                }
            })
        }));


        client
    }

    fn config(&self) -> &Config {
        &self.0.config
    }

    pub fn status(&self) -> SnStatus {
        self.0.state.read().unwrap().sn_status
    }


    async fn start(&self) {
        loop {
            let now = bucky_time_now();
            enum NextStep {
                Break,
                Wait(Duration),  
                SendPing(SendPingOptions, Duration), 
            };

            let next_step = {
                let mut state = self.0.state.write().unwrap();
                let ping_interval = match &mut state.sn_status => {
                    SnStatus::Connecting => self.config().ping_interval_init, 
                    SnStatus::Online(last_resp_time) => {
                        if now > *last_resp_time && Duration::from_micros(now - *last_resp_time) > self.config().offline() {
                            state.sn_status = SnStatus::Offline;
                        }
                        self.config().ping_interval
                    },
                    SnStatus::Offline => self.config().ping_interval
                };
                    
                match &mut state.client_status {
                    ClientStatus::Stopped => NextStep::Break, 
                    ClientStatus::Running {last_ping_time, last_update_seq} => {
                        if now > *last_ping_time && Duration::from_micros(now - *last_ping_time) > ping_interval {
                            *last_ping_time = now;
                            let seq = self.0.seq_genarator.generate()
                            let with_device = if let Some(last_update_seq) = last_update_seq {
                                seq > *last_update_seq
                            } else {
                                false
                            };
                            NextStep::SendPing(SendPingOptions {
                                seq, 
                                with_device
                            }, ping_interval / 2)
                        } else {
                            NextStep::Wait(ping_interval / 2)
                        }
                    },
                }            
            };

            match next_step {
                NextStep::Break => {
                    break;
                }, 
                NextStep::Wait(interval) => {
                    future::timeout(interval, future::pending::<()>()).await;
                },
                NextStep::SendPing(options, interval) => {
                    self.send_ping_inner(options).await;
                    future::timeout(interval, future::pending::<()>()).await;
                }
            }
        }
        
    }

    fn stop(&self) {
        self.0.state.write().unwrap().client_status = ClientStatus::Stopped;
    }

    fn on_local_updated(&self) {
        let mut state = self.0.state.write().unwrap();
        match &mut state.client_status {
            ClientStatus::Stopped => {}, 
            ClientStatus::Running { last_update_seq, .. } => {
                if last_update_seq.is_none() {
                    last_update_seq = Some(self.0.seq_genarator.generate());
                }
            },
        }            
    }

    pub fn send_ping(&self) {
        let options = {
            let mut state = self.0.state.write().unwrap();
        
            match &mut state.client_status {
                ClientStatus::Stopped => None, 
                ClientStatus::Running {last_ping_time, last_update_seq} => {
                    *last_ping_time = bucky_time_now();
                    let seq = self.0.seq_genarator.generate()
                    let with_device = if let Some(last_update_seq) = last_update_seq {
                        seq > *last_update_seq
                    } else {
                        false
                    };
                    Some(SendPingOptions {
                        seq, 
                        with_device
                    })
                },
            }
        };
       
        if let Some(options) = options {
            let client = self.clone();
            task::spawn(async move {
                client.send_ping_inner(options).await;
            });
        }
    }


    pub fn on_called(&self, called: &SnCalled, key: &MixAesKey, call_seq: TempSeq, call_time: Timestamp, from: &Endpoint, from_interface: Interface) {
        let resp = SnCalledResp {
            seq: called.seq,
            result: 0,
            sn_peer_id: self.sn_peerid.clone(),
        };

        let mut pkg_box = PackageBox::encrypt_box(
            self.sn_peerid.clone(), 
            key.clone());
        pkg_box.push(resp);

        let mut context = PackageBoxEncodeContext::default();
        let _ = from_interface.send_box_to(&mut context, &pkg_box, from)?;

        Ok(())
    }

    fn on_ping_resp(&self) {
        // if new_endpoint > UpdateOuterResult::None {
        //     log::info!("{} ping-resp, sn: {}/{} get new endpoint will update the desc and resend ping.", self, resp.sn_peer_id.to_string(), from.to_string());

        //     let stack = Stack::from(&self.env.stack);
        //     let clients: Vec<Arc<Client>> = self.clients.read().unwrap().iter().map(|c| c.1.clone()).collect();
        //     async_std::task::spawn(async move {
        //         if new_endpoint == UpdateOuterResult::Update {
        //             stack.update_local().await;
        //         } else if new_endpoint == UpdateOuterResult::Reset {
        //             stack.reset_local().await;
        //         }
        //         for client in clients {
        //             client.local_updated();
        //             client.send_ping();
        //         }
        //     });
        // } else if is_resend_immediate {
        //     client.unwrap().send_ping();
        // }
    }

    fn on_udp_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface) -> (UpdateOuterResult/*has new endpoint*/, bool/*is_resend_immdiate*/) {
        let now = bucky_time_now();
        self.last_resp_time.store(now, atomic::Ordering::Release);

        let mut rto = 0;
        let mut is_handled = false;

        let active_session_index = self.active_session_index.load(atomic::Ordering::Acquire) as usize;
        let sessions = self.sessions.read().unwrap();
        let try_session = sessions.get(active_session_index);
        let mut new_endpoint = UpdateOuterResult::None;
        if let Some(s) = try_session {
            let r = s.on_ping_resp(resp, from, from_interface.clone(), now, &mut rto, &mut is_handled);
            new_endpoint = std::cmp::max(r, new_endpoint);
        }

        if !is_handled {
            let mut index = 0;
            for session in (*sessions).as_slice() {
                let r = session.on_ping_resp(resp, from, from_interface.clone(), now, &mut rto, &mut is_handled);
                new_endpoint = std::cmp::max(r, new_endpoint);
                if is_handled {
                    let _ = self.active_session_index.compare_exchange(std::u32::MAX, index, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst);
                    break;
                }
                index += 1;
            }
        }


        // if !self.interface.is_same(&from_interface) {
        //     *is_handled = false;
        //     return UpdateOuterResult::None;
        // }
        // *is_handled = true;

        // if resp.end_point_array.len() == 0 {
        //     return UpdateOuterResult::None;
        // }

        // let is_remote_none = self.remote_endpoint.read().unwrap().is_none();
        // if is_remote_none {
        //     *self.remote_endpoint.write().unwrap() = Some(from.clone());
        // }
        // self.last_resp_time.store(now, atomic::Ordering::Release);
        // let last_ping_time = self.last_ping_time.load(atomic::Ordering::Acquire);
        // if now > last_ping_time {
        //     *rto = (now - last_ping_time) as u16;
        // } else {
        //     *rto = 0;
        // }

        // let out_endpoint = resp.end_point_array.get(0).unwrap();
        // self.net_listener.update_outer(&self.interface.local(), &out_endpoint)
    

        self.update_status();

        let is_resend_immdiate = {
            let _ = self.last_update_seq.compare_exchange(resp.seq.value(), 0, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst);
            false
        };

        (new_endpoint, is_resend_immdiate)
    }

    async fn send_ping_inner(&self, options: SendPingOptions) -> BuckyResult<()> {
        // let now = self.create_time.elapsed().as_millis() as u64;
        // let now_abs = SystemTime::now();
        // let now_abs_u64 = bucky_time_now();
        // let seq = self.seq_genarator.generate();

        // let stack = Stack::from(&self.env.stack);
        // let local_peer = stack.device_cache().local();
        
        // let last_resp_time = self.last_resp_time.load(atomic::Ordering::Acquire);
        // let to_session_index = if last_resp_time == 0 || (now < last_resp_time || now - last_resp_time > 1000) {
        //     self.active_session_index.store(std::u32::MAX, atomic::Ordering::Release);
        //     std::u32::MAX
        // } else {
        //     self.active_session_index.load(atomic::Ordering::Acquire)
        // };

        // let to_sessions = {
        //     let sessions = self.sessions.read().unwrap();
        //     let to_session = if to_session_index != std::u32::MAX {
        //         sessions.get(to_session_index as usize)
        //     } else {
        //         None
        //     };

        //     match to_session {
        //         Some(s) => vec![s.prepare_send_endpoints(now_abs_u64)],
        //         None => (*sessions).iter().map(|s| s.prepare_send_endpoints(now_abs_u64)).collect()
        //     }
        // };

        // if to_sessions.len() == 0 {
        //     return Err(BuckyError::new(BuckyErrorCode::NotFound, "no ping target"));
        // }

        // let ping_pkg = {
        //     let stack = Stack::from(&self.env.stack);
        //     let last_update_seq = self.last_update_seq.swap(seq.value(), atomic::Ordering::AcqRel);
        //     let mut ping_pkg = SnPing {
        //         protocol_version: 0, 
        //         stack_version: 0, 
        //         seq,
        //         from_peer_id: Some(stack.local_device_id().clone()),
        //         sn_peer_id: self.sn_peerid.clone(),
        //         peer_info: if last_update_seq != 0 { Some(local_peer.clone()) } else { None }, // 本地信息更新了信息，需要同步，或者服务器要求更新
        //         send_time: now_abs_u64,
        //         contract_id: None, // <TODO>
        //         receipt: None
        //     };

        //     match &ping_pkg.peer_info {
        //         Some(dev) => log::info!("{} ping-req seq: {:?}, endpoints: {:?}", self,  ping_pkg.seq, dev.connect_info().endpoints()),
        //         None => log::debug!("{} ping-req seq: {:?}, endpoints: none", self, ping_pkg.seq),
        //     }
        //     // 填充receipt
        //     self.contract.prepare_receipt(&mut ping_pkg, now_abs, stack.keystore().private_key());
        //     ping_pkg
        // };

        // let key_stub = stack.keystore().create_key(self.sn.desc(), true);

        // let mut pkg_box = PackageBox::encrypt_box(
        //     self.sn_peerid.clone(), 
        //     key_stub.key.clone());

        // if let keystore::EncryptedKey::Unconfirmed(key_encrypted) = key_stub.encrypted {
        //     let stack = Stack::from(&self.env.stack);
        //     let mut exchg = Exchange::from((&ping_pkg, local_peer.clone(), key_encrypted, key_stub.key.mix_key));
        //     let _ = exchg.sign(stack.keystore().signer()).await;
        //     pkg_box.push(exchg);
        // }

        // let ping_seq = ping_pkg.seq.clone();
        // pkg_box.push(ping_pkg);

        // let mut context = PackageBoxEncodeContext::default();

        // self.last_ping_time.store(now, atomic::Ordering::Release);

        // self.contract.will_ping(seq.value());

        // struct SendIter {
        //     sessions: Vec<(Interface, Vec<Endpoint>)>,
        //     sub_pos: usize,
        //     pos: usize,
        // }

        // impl Iterator for SendIter {
        //     type Item = (Interface, Endpoint);

        //     fn next(&mut self) -> Option<Self::Item> {
        //         let sessions = self.sessions.get(self.pos);
        //         if let Some((from, to_endpoints)) = sessions {
        //             let ep = to_endpoints.get(self.sub_pos);
        //             if let Some(ep) = ep {
        //                 self.sub_pos += 1;
        //                 Some(((*from).clone(), ep.clone()))
        //             } else {
        //                 self.pos += 1;
        //                 self.sub_pos = 0;
        //                 self.next()
        //             }
        //         } else {
        //             None
        //         }
        //     }
        // }

        // let send_iter = SendIter {
        //     sessions: to_sessions,
        //     sub_pos: 0,
        //     pos: 0
        // };
        // Interface::send_box_mult(&mut context,
        //                                  &pkg_box,
        //                                  send_iter,
        //                                  |from, to, result| {
        //                                      log::debug!("{} ping seq:{:?} from {} to {}/{}, result: {:?}", self, ping_seq, from.local(), self.sn_peerid.to_string(), to, result);
        //                                      true
        //                                  })?;
        Ok(())
    }
}


trait Session {

}

struct UdpSession {
    local: Interface,
    endpoints: Vec<Endpoint>, 
}

impl UdpSession {
    fn new(local: Interface, sn: &Device) -> Self {
        let endpoints = sn.connect_info().endpoints().iter()
            .filter(|ep| ep.is_same_ip_version(&local.local()) && ep.is_udp()).map(|ep| ep.clone()).collect();
        Self {
            local, 
            endpoints, 
        }
    }

    fn prepare_send_endpoints(&self, now: u64) -> (Interface, Vec<Endpoint>) {
        let local_endpoint = self.interface.local();
        let to_endpoints = {
            let (eps, is_reset) = {
                let remote_endpoint = self.remote_endpoint.read().unwrap();
                let last_resp_time = self.last_resp_time.load(atomic::Ordering::Acquire);
                let last_ping_time = self.last_ping_time.load(atomic::Ordering::Acquire);
                if last_resp_time == 0 || remote_endpoint.is_none() || (now > last_ping_time && now - last_ping_time > 1000 && last_ping_time > last_resp_time) {
                    (self.sn.connect_info().endpoints().iter().filter(|ep| ep.is_same_ip_version(&local_endpoint) && ep.is_udp()).map(|ep| ep.clone()).collect(),
                    true)
                } else {
                    (vec![remote_endpoint.unwrap().clone()],
                    false)
                }
            };

            if is_reset {
                *self.remote_endpoint.write().unwrap() = None;
            }
            eps
        };

        self.last_ping_time.store(now, atomic::Ordering::Release);

        (self.interface.clone(), to_endpoints)
    }

    fn on_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface, now: u64, rto: &mut u16, is_handled: &mut bool) -> UpdateOuterResult {
        //TODO
    }
        
}


fn is_new_endpoint(desc: &Device, ep: &Endpoint) -> bool {
    for cur in desc.connect_info().endpoints() {
        if cur.is_udp() && cur.addr() == ep.addr() {
            return false;
        }
    }
    true
}
