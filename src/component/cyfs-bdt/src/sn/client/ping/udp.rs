use log::*;
use std::{
    sync::{Arc, RwLock,}, 
    time::{Duration}
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
    manager::{PingClient, SnStatus},
};

#[derive(Clone)]
pub struct Config {
    pub ping_interval_init: Duration,
    pub ping_interval: Duration,
    pub offline: Duration,
}


#[derive(Debug, Clone)]
enum ClientStatus {
    Running {
        last_ping_time: Timestamp,
        last_update_seq: Option<TempSeq>,
    },
    Stopped
}

struct ClientState {
    sessions: Vec<UdpSession>,
    client_status: ClientStatus,
    sn_status: SnStatus
}

struct ClientInner {
    stack: WeakStack, 
    config: Config, 
    sn_id: DeviceId,
    sn: Device, 
    net_listener: NetListener, 
    seq_genarator: TempSeqGenerator,

    state: RwLock<ClientState>    
}

#[derive(Debug)]
struct SendPingOptions {
    seq: TempSeq, 
    with_device: bool, 
    sessions: Vec<(Interface, Vec<Endpoint>)>
}

struct SendPingIter {
    sessions: Vec<(Interface, Vec<Endpoint>)>, 
    sub_pos: usize,
    pos: usize,
}

impl Into<SendPingIter> for SendPingOptions {
    fn into(self) -> SendPingIter {
        SendPingIter {
            sessions: self.sessions, 
            sub_pos: 0,
            pos: 0
        }
    }
}


impl Iterator for SendPingIter {
    type Item = (Interface, Endpoint);

    fn next(&mut self) -> Option<Self::Item> {
        let sessions = self.sessions.get(self.pos);
        if let Some((from, to_endpoints)) = sessions {
            let ep = to_endpoints.get(self.sub_pos);
            if let Some(ep) = ep {
                self.sub_pos += 1;
                Some(((*from).clone(), ep.clone()))
            } else {
                self.pos += 1;
                self.sub_pos = 0;
                self.next()
            }
        } else {
            None
        }
    }
}


#[derive(Clone)]
pub(super) struct UdpClient(Arc<ClientInner>);

impl std::fmt::Display for UdpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stack = Stack::from(&self.0.stack);
        write!(f, "UdpPingClient{{local:{}, sn:{}}}", stack.local_device_id(), self.sn())
    }
}


impl PingClient for UdpClient {
    fn sn(&self) -> &DeviceId {
        &self.0.sn_id
    }

    fn clone_as_ping_client(&self) -> Box<dyn PingClient> {
        Box::new(self.clone())
    }

    fn status(&self) -> SnStatus {
        self.0.state.read().unwrap().sn_status
    }

    fn start(&self) {
        info!("{} starting", self);
        let client = self.clone();
        task::spawn(async move {
            client.start_inner().await;
        });
    }

    fn stop(&self) {
        self.0.state.write().unwrap().client_status = ClientStatus::Stopped;
        info!("{} stoped", self);
    }

    
    fn on_udp_ping_resp(&self, resp: &SnPingResp, _from: &Endpoint, interface: Interface) -> BuckyResult<()> {
        let now = bucky_time_now();
        
        let handled = {
            let mut state = self.0.state.write().unwrap();
            if let Some(session) = state.sessions.iter_mut().find(|s| s.local.is_same(&interface)) {
                if session.last_resp_time < now {
                    session.last_resp_time = now;
                } 
                let handled = match &mut state.client_status {
                    ClientStatus::Running { last_update_seq, .. } => {
                        if last_update_seq.and_then(|seq| if seq < resp.seq { Some(()) } else { None }).is_some() {
                            *last_update_seq = None;    
                        } 
                        false
                    }, 
                    ClientStatus::Stopped => true
                };
                if !handled {
                    match &mut state.sn_status {
                        SnStatus::Connecting => {
                            state.sn_status = SnStatus::Online(now);
                        } 
                        SnStatus::Online(last_resp) => {
                            if *last_resp < now {
                                *last_resp = now;
                            }
                        } 
                        SnStatus::Offline => {
                            state.sn_status = SnStatus::Online(now);
                        }            
                    }
                }
                false
            } else {
                true
            }
        };

        if handled {
            return Ok(());
        }
        
        let update = {
            if resp.result == BuckyErrorCode::NotFound.as_u8() {
                Some(UpdateOuterResult::None)
            } else if resp.end_point_array.len() > 0 {
                let out_endpoint = resp.end_point_array.get(0).unwrap();
                let result = self.0.net_listener.update_outer(&interface.local(), &out_endpoint);
                if result > UpdateOuterResult::None {
                   Some(result)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(result) = update {
            let client = self.clone();
            task::spawn(async move {
                client.on_outer_updated(result).await;
            });
        }
       
        Ok(())
    }
}



impl UdpClient {
    pub fn new(stack: WeakStack, config: Config, sn: Device, net_listener: NetListener) -> Self {
        let sn_id = sn.desc().device_id();

        let mut sessions = Vec::default();

        for udp in net_listener.udp() {
            if let Some(session) = UdpSession::new(udp.clone(), &sn) {
                sessions.push(session);
            }   
        }

        let seq_genarator = TempSeqGenerator::new();
        let next_seq = seq_genarator.generate();
        let client = Self(Arc::new(ClientInner {
            stack, 
            config, 
            sn_id,
            sn, 
            net_listener, 
            seq_genarator, 
            state: RwLock::new(ClientState {
                sessions,  
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


    async fn start_inner(&self) {
        loop {
            let now = bucky_time_now();
            enum NextStep {
                Break,
                Wait(Duration),  
                SendPing(SendPingOptions, Duration), 
            }

            let next_step = {
                let mut state = self.0.state.write().unwrap();
                let (ping_interval, sessions) = match &mut state.sn_status {
                    SnStatus::Connecting => (self.config().ping_interval_init, UdpSession::sessions(state.sessions.iter(), UdpSessionFilter::All)), 
                    SnStatus::Online(last_resp_time) => {
                        if now > *last_resp_time && Duration::from_micros(now - *last_resp_time) > self.config().offline {
                            state.sn_status = SnStatus::Offline;
                        }
                        (self.config().ping_interval, UdpSession::sessions(state.sessions.iter(), UdpSessionFilter::Active(self.config().offline)))
                    },
                    SnStatus::Offline => (self.config().ping_interval, UdpSession::sessions(state.sessions.iter(), UdpSessionFilter::All))
                };
                    
                match &mut state.client_status {
                    ClientStatus::Stopped => NextStep::Break, 
                    ClientStatus::Running {last_ping_time, last_update_seq} => {
                        if now > *last_ping_time && Duration::from_micros(now - *last_ping_time) > ping_interval {
                            *last_ping_time = now;
                            let seq = self.0.seq_genarator.generate();
                            let with_device = if let Some(last_update_seq) = last_update_seq {
                                seq > *last_update_seq
                            } else {
                                false
                            };
                            NextStep::SendPing(SendPingOptions {
                                seq, 
                                with_device, 
                                sessions
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
                    let _ = future::timeout(interval, future::pending::<()>()).await;
                },
                NextStep::SendPing(options, interval) => {
                    let _ = self.send_ping_inner(options).await;
                    let _ = future::timeout(interval, future::pending::<()>()).await;
                }
            }
        }
        
    }

    pub fn send_ping(&self) {
        let options = {
            let mut state = self.0.state.write().unwrap();
        
            match &mut state.client_status {
                ClientStatus::Stopped => None, 
                ClientStatus::Running {last_ping_time, last_update_seq} => {
                    *last_ping_time = bucky_time_now();
                    let seq = self.0.seq_genarator.generate();
                    let with_device = if let Some(last_update_seq) = last_update_seq {
                        seq > *last_update_seq
                    } else {
                        false
                    };
                    Some(SendPingOptions {
                        seq, 
                        with_device, 
                        sessions: UdpSession::sessions(state.sessions.iter(), UdpSessionFilter::Latest)
                    })
                },
            }
        };
       
        if let Some(options) = options {
            let client = self.clone();
            task::spawn(async move {
                let _ = client.send_ping_inner(options).await;
            });
        }
    }

    async fn on_outer_updated(&self, result: UpdateOuterResult) {
        let stack = Stack::from(&self.0.stack);
        if result == UpdateOuterResult::Update {
            stack.update_local().await;
        } else if result == UpdateOuterResult::Reset {
            stack.reset_local().await;
        } else {
            unreachable!();
        }

        let options = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.client_status {
                ClientStatus::Stopped => None, 
                ClientStatus::Running {last_ping_time, last_update_seq} => {
                    *last_ping_time = bucky_time_now();
                    let seq = self.0.seq_genarator.generate();
                    *last_update_seq = Some(seq);
                    Some(SendPingOptions {
                        seq, 
                        with_device: true, 
                        sessions: UdpSession::sessions(state.sessions.iter(), UdpSessionFilter::Latest)
                    })
                },
            }
        };

        if let Some(options) = options {
            let _ = self.send_ping_inner(options).await;
        }
    }


    async fn send_ping_inner(&self, options: SendPingOptions) -> BuckyResult<()> {
        let stack = Stack::from(&self.0.stack);
        let local_device = stack.device_cache().local().clone();
        let seq = options.seq; 
        let ping_pkg = SnPing {
            protocol_version: 0, 
            stack_version: 0, 
            seq,
            from_peer_id: Some(stack.local_device_id().clone()),
            sn_peer_id: self.sn().clone(),
            peer_info: if options.with_device { Some(local_device.clone()) } else { None }, 
            send_time: bucky_time_now(),
            contract_id: None, 
            receipt: None
        };

        let key_stub = stack.keystore().create_key(self.0.sn.desc(), true);

        let mut pkg_box = PackageBox::encrypt_box(
            self.sn().clone(), 
            key_stub.key.clone());

        if let keystore::EncryptedKey::Unconfirmed(key_encrypted) = key_stub.encrypted {
            let mut exchg = Exchange::from((&ping_pkg, local_device.clone(), key_encrypted, key_stub.key.mix_key));
            let _ = exchg.sign(stack.keystore().signer()).await;
            pkg_box.push(exchg);
        }
        pkg_box.push(ping_pkg);
        
        info!("{} send sn ping, options={:?}", self, options);
        let mut context = PackageBoxEncodeContext::default();
        let iter: SendPingIter = options.into();
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


struct UdpSession {
    local: Interface,
    endpoints: Vec<Endpoint>, 
    last_resp_time: Timestamp    
}

enum UdpSessionFilter {
    All, 
    Active(Duration), 
    Latest, 
}

impl UdpSession {
    fn new(local: Interface, sn: &Device) -> Option<Self> {
        let endpoints: Vec<Endpoint> = sn.connect_info().endpoints().iter()
            .filter(|ep| ep.is_same_ip_version(&local.local()) && ep.is_udp()).map(|ep| ep.clone()).collect();
        if endpoints.len() > 0 {
            Some(Self {
                local, 
                endpoints, 
                last_resp_time: 0
            })
        } else {
            None
        }
    }

    fn sessions<'a>(iter: impl Iterator<Item=&'a Self>, filter: UdpSessionFilter) -> Vec<(Interface, Vec<Endpoint>)> {
        match filter {
            UdpSessionFilter::All => iter.map(|session| (session.local.clone(), session.endpoints.clone())).collect(), 
            UdpSessionFilter::Active(timeout) => {
                let now = bucky_time_now();
                iter.filter(|session|  !(now > session.last_resp_time && Duration::from_micros(now - session.last_resp_time) > timeout))
                    .map(|session| (session.local.clone(), session.endpoints.clone())).collect()
            },
            UdpSessionFilter::Latest => {
                if let Some(session) = iter.max_by(|l, r| l.last_resp_time.cmp(&r.last_resp_time)) {
                    vec![(session.local.clone(), session.endpoints.clone())]
                } else {
                    vec![]
                }
            }
        }
    }
}


