
use log::*;
use std::{
    sync::{Arc, RwLock,}, 
    collections::{LinkedList}, 
    time::Duration, 
};
use futures::future::{AbortRegistration, join_all};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::{NetListener, udp::{Interface, PackageBoxEncodeContext}}, 
    stack::{WeakStack, Stack},
    dht::*
};
use super::super::{
    manager::PingClientCalledEvent
};
use super::{
    udp::{self, *}
};

#[derive(Clone)]
pub struct Config {
    pub interval: Duration, 
    pub udp: udp::Config
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SnStatus {
    Connecting, 
    Online, 
    Offline
}

#[derive(Clone)]
pub struct PingSessionResp {
    from: Endpoint, 
    err: BuckyErrorCode, 
    endpoints: Vec<Endpoint>
}


#[async_trait::async_trait]
pub trait PingSession: Send + Sync {
    fn sn(&self) -> &DeviceId;
    fn local(&self) -> Endpoint;
    fn reset(&self) -> Box<dyn PingSession>;
    fn clone_as_ping_session(&self) -> Box<dyn PingSession>;
    fn on_time_escape(&self, now: Timestamp);
    async fn wait(&self) -> BuckyResult<PingSessionResp>;
    fn stop(&self);
    fn on_udp_ping_resp(&self, resp: &SnPingResp, from: &Endpoint) -> BuckyResult<()>;
}


enum ActiveState {
    FirstTry(Box<dyn PingSession>), 
    SecondTry(Box<dyn PingSession>), 
    Wait(Timestamp, Box<dyn PingSession>)
}

enum ClientState {
    Init(StateWaiter), 
    Connecting {
        waiter: StateWaiter, 
        sessions: Vec<Box<dyn PingSession>>, 
    }, 
    Active {
        waiter: StateWaiter, 
        state: ActiveState
    }, 
    Timeout, 
    Stopped
}

struct ClientImpl {
    stack: WeakStack, 
    config: Config, 
    sn_id: DeviceId, 
    sn: Device, 
    gen_seq: Arc<TempSeqGenerator>, 
    net_listener: NetListener, 
    local_device: Device,  
    state: RwLock<ClientState>
}

#[derive(Clone)]
pub struct PingClient(Arc<ClientImpl>);

impl PingClient {
    pub(crate) fn new(
        stack: WeakStack, 
        config: Config, 
        gen_seq: Arc<TempSeqGenerator>, 
        net_listener: NetListener, 
        sn: Device, 
        local_device: Device, 
    ) -> Self {
        let strong_stack = Stack::from(&stack);
        let sn_id = sn.desc().device_id();
        strong_stack.keystore().reset_peer(&sn_id);
    
        Self(Arc::new(ClientImpl {
            stack, 
            config, 
            gen_seq, 
            net_listener, 
            sn, 
            sn_id, 
            local_device, 
            state: RwLock::new(ClientState::Init(StateWaiter::new()))
        }))
    }


    pub fn stop(&self) {
        let (waiter, sessions) = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                ClientState::Init(waiter) => {
                    let waiter = waiter.transfer();
                    *state = ClientState::Stopped;
                    (Some(waiter), vec![])
                }, 
                ClientState::Connecting {
                    waiter, 
                    sessions
                } => {
                    let waiter = waiter.transfer();
                    let sessions = sessions.iter().map(|s| s.clone_as_ping_session()).collect();
                    *state = ClientState::Stopped;
                    (Some(waiter), sessions)
                },
                ClientState::Active {
                    waiter, 
                    state
                } => {
                    let waiter = waiter.transfer();
                    let sessions = match state {
                        ActiveState::FirstTry(session) => vec![session.clone_as_ping_session()], 
                        ActiveState::SecondTry(session) => vec![session.clone_as_ping_session()], 
                        _ => vec![]
                    };
                    *state = ClientState::Stopped;
                    (Some(waiter), sessions)
                },
                _ => (None, vec![])
            }
        };

        if let Some(waiter) = waiter {
            waiter.wake()
        };

        for session in sessions {
            session.stop();
        }
        
    }


    pub fn sn(&self) -> &DeviceId {
        &self.0.sn_id
    }


    fn sync_session_resp(&self, session: Box<dyn PingSession>, result: BuckyResult<PingSessionResp>) {
        unimplemented!()
    }

    pub async fn wait_offline(&self) -> BuckyResult<()> {
        enum NextStep {
            Wait(AbortRegistration),
            Return(BuckyResult<()>)
        }

        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                ClientState::Stopped => NextStep::Return(Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"))), 
                ClientState::Active {
                    waiter, 
                    ..
                } => NextStep::Wait(waiter.new_waiter()), 
                ClientState::Timeout =>  NextStep::Return(Ok(())), 
            }
        };
       
        match next {
            NextStep::Return(result) => result, 
            NextStep::Wait(waiter) => {
                StateWaiter::wait(waiter, || {
                    let state = self.0.state.read().unwrap();
                    match &*state {
                        ClientState::Stopped => NextStep::Return(Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"))), 
                        ClientState::Timeout =>  NextStep::Return(Ok(())), 
                        _ => unreachable!()
                    }
                }).await
            }
        }
    }

    pub async fn wait_online(&self) -> BuckyResult<SnStatus> {
        enum NextStep {
            Wait(AbortRegistration),
            Start(AbortRegistration), 
            Return(BuckyResult<SnStatus>)
        }
        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                ClientState::Init(waiter) => {
                    let waiter = waiter.new_waiter();
                    NextStep::Start(waiter)
                }, 
                ClientState::Connecting{ waiter, ..} => NextStep::Wait(waiter.new_waiter()), 
                ClientState::Active(_) => NextStep::Return(Ok(SnStatus::Online)), 
                ClientState::Timeout =>  NextStep::Return(Ok(SnStatus::Offline)), 
                ClientState::Stopped => NextStep::Return(Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"))), 
            }
        };
       
        let state = || {
            let state = self.0.state.read().unwrap();
            match &*state {
                ClientState::Active(_) => Ok(SnStatus::Online), 
                ClientState::Timeout =>  Ok(SnStatus::Offline), 
                ClientState::Stopped => Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled")), 
                _ => unreachable!()
            }
        };

        match next {
            NextStep::Return(result) => result, 
            NextStep::Wait(waiter) => StateWaiter::wait(waiter, state).await, 
            NextStep::Start(waiter) => {
                let mut sessions = vec![];
                for local in self.0.net_listener.udp().iter().filter(|interface| interface.local().addr().is_ipv4()) {
                    let sn_endpoints = self.0.sn.connect_info().endpoints().iter().filter(|endpoint| endpoint.is_udp() && endpoint.is_same_ip_version(local)).cloned().collect();
                    if sn_endpoints.len() > 0 {
                        let params = UdpSesssionParams {
                            config: self.0.config.udp.clone(), 
                            local: local.clone(),
                            local_device: self.0.local_device.clone(), 
                            with_device: true, 
                            sn_desc: self.0.sn.desc().clone(),
                            sn_endpoints,  
                        };
                        sessions.push(UdpPingSession::new(self.0.stack.clone(), self.0.gen_seq.clone(), params));
                    }
                };

                // if sessions.len() == 0 {
                //     for local in net_listener.tcp().iter().filter(|listener| {
                //         listener.local().addr().is_ipv4() 
                //             && (listener.mapping_port().is_some() 
                //                 || listener.outer().and_then(|ep| if ep.is_static_wan() { Some(ep) } else { None }).is_some())
                //             }) {
                //         let sn_endpoints = sn.connect_info().endpoints().iter().filter(|endpoint|endpoint.is_tcp() && endpoint.is_same_ip_version(local)).cloned().collect();
        
                //         if sn_endpoints.len() > 0 {
                //             let params = UdpSesssionParams {
                //                 config: config.udp.clone(), 
                //                 local: local.clone(),
                //                 local_device: local_device.clone(), 
                //                 with_device: true, 
                //                 sn_desc: sn.desc().clone(),
                //                 sn_endpoints,  
                //             };
                //             sessions.push(UdpPingSession::new(stack.clone(),  gen_seq.clone(),  params));
                //         }
                //     }
                // }

                let start = {
                    let mut state = self.0.state.write().unwrap();
                    match &mut *state {
                        ClientState::Init(waiter) => {
                            let waiter = waiter.transfer();
                            *state = ClientState::Connecting {
                                waiter, 
                                sessions: sessions.iter().map(|s| s.clone_as_ping_session()).collect(), 
                            };
                            true
                        },
                        _ => false
                    }
                };

                if start {
                    for session in sessions.into_iter() {
                        let client = self.clone();
                        task::spawn(async move {
                            let result = session.wait().await;
                            client.sync_session_resp(session, result);
                        })
                    }
                } 

                StateWaiter::wait(waiter, state).await
            }
        }
       
    }

    pub fn on_time_escape(&self, now: Timestamp) {
        let sessions = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                ClientState::Connecting {
                    sessions, 
                    ..
                } => sessions.iter().map(|session| session.clone_as_ping_session()).collect(), 
                ClientState::Active { 
                    active, 
                    .. 
                } => {
                    match active {
                        ActiveState::Wait(next_time, session) => {
                            if now > *next_time {
                                let session = session.clone_as_ping_session();
                                *active = ActiveState::FirstTry(session.clone_as_ping_session());
                                let client = self.clone();
                                task::spawn(async move {
                                    self.sync_session_resp(session.clone_as_ping_session(), session.wait().await);
                                });
                                vec![session]
                            } else {
                                vec![]
                            }
                        }, 
                        ActiveState::FirstTry(session) => vec![session.clone_as_ping_session()], 
                        ActiveState::SecondTry(session) => vec![session.clone_as_ping_session()], 
                    }
                }, 
                _ => vec![]
            }
        };
        
        for session in sessions {
            session.on_time_escape(now);
        }
    }

    pub fn on_udp_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, interface: Interface) {
        let sessions = {
            let state = self.0.state.read().unwrap();
            match &*state {
                ClientState::Connecting {
                    sessions, 
                    ..
                } => sessions.iter().filter_map(|session| {
                    if session.local() == interface.local() {
                        Some(session.clone_as_ping_session())
                    } else {
                        None
                    }
                }).collect(), 
                ClientState::Active { 
                    active, 
                    .. 
                } => {
                    match active {
                        ActiveState::FirstTry(session) => {
                            if session.local() == interface.local() {
                                vec![session.clone_as_ping_session()]
                            } else {
                                vec![]
                            }
                        }, 
                        ActiveState::SecondTry(session) => {
                            if session.local() == interface.local() {
                                vec![session.clone_as_ping_session()]
                            } else {
                                vec![]
                            }
                        }, 
                        _ => vec![]
                    }
                }, 
                _ => vec![]
            }
        };

        for session in sessions {
            session.on_udp_ping_resp(resp, from);
        }
    }


}




