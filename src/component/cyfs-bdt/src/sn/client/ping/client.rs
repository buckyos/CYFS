
// use log::*;
use std::{
    sync::{Arc, RwLock,}, 
    time::Duration, 
};
use async_std::{
    task
};
use futures::future::AbortRegistration;
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{v0::*}, 
    interface::{*, udp::{Interface}}, 
    stack::{WeakStack, Stack},
};
use super::{
    udp::{self, *}
};

#[derive(Clone)]
pub struct PingConfig {
    pub interval: Duration, 
    pub udp: udp::Config
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SnStatus {
    Online, 
    Offline
}



impl std::fmt::Display for SnStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = match self {
            Self::Online => "online",
            Self::Offline => "offline",
        };

        write!(f, "{}", v)
    }
}


impl std::str::FromStr for SnStatus {
    type Err = BuckyError;

    fn from_str(s: &str) -> BuckyResult<Self> {
        match s {
            "online" => Ok(Self::Online),
            "offline" => Ok(Self::Offline),
            _ => {
                let msg = format!("unknown SnStatus value: {}", s);
                log::error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidData, msg))
            }
        }
    }
}


#[derive(Clone, Debug)]
pub struct PingSessionResp {
    pub from: Endpoint, 
    pub err: BuckyErrorCode, 
    pub endpoints: Vec<Endpoint>
}


#[async_trait::async_trait]
pub trait PingSession: Send + Sync + std::fmt::Display {
    fn sn(&self) -> &DeviceId;
    fn local(&self) -> Endpoint;
    fn reset(&self,  local_device: Option<Device>, sn_endpoint: Option<Endpoint>) -> Box<dyn PingSession>;
    fn clone_as_ping_session(&self) -> Box<dyn PingSession>;
    async fn wait(&self) -> BuckyResult<PingSessionResp>;
    fn stop(&self);
    fn on_time_escape(&self, _now: Timestamp) {

    }
    fn on_udp_ping_resp(&self, _resp: &SnPingResp, _from: &Endpoint) -> BuckyResult<()> {
        Ok(())
    }
}


enum ActiveState {
    FirstTry(Box<dyn PingSession>), 
    SecondTry(Box<dyn PingSession>), 
    Wait(Timestamp, Box<dyn PingSession>)
}

impl ActiveState {
    fn cur_session(&self) -> Option<Box<dyn PingSession>> {
        match self {
            Self::FirstTry(session) => Some(session.clone_as_ping_session()), 
            Self::SecondTry(session) => Some(session.clone_as_ping_session()),
            _ => None 
        } 
    }
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
    config: PingConfig, 
    sn_index: usize,  
    sn_id: DeviceId, 
    sn: Device, 
    gen_seq: Arc<TempSeqGenerator>, 
    net_listener: NetListener, 
    local_device: RwLock<Device>,  
    state: RwLock<ClientState>
}

#[derive(Clone)]
pub struct PingClient(Arc<ClientImpl>);

impl std::fmt::Display for PingClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stack = Stack::from(&self.0.stack);
        write!(f, "PingClients{{local:{}, sn:{}}}", stack.local_device_id(), self.sn())
    }
}

impl PingClient {
    pub(crate) fn new(
        stack: WeakStack, 
        config: PingConfig, 
        gen_seq: Arc<TempSeqGenerator>, 
        net_listener: NetListener, 
        sn_index: usize, 
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
            sn_index, 
            local_device: RwLock::new(local_device), 
            state: RwLock::new(ClientState::Init(StateWaiter::new()))
        }))
    }

    pub(crate) fn reset(
        &self, 
        net_listener: NetListener, 
        local_device: Device, 
    ) -> Self {
        Self(Arc::new(ClientImpl {
            stack: self.0.stack.clone(), 
            config: self.0.config.clone(), 
            sn_id: self.0.sn_id.clone(),
            sn_index: self.0.sn_index, 
            sn: self.0.sn.clone(), 
            gen_seq: self.0.gen_seq.clone(), 
            net_listener, 
            local_device: RwLock::new(local_device), 
            state: RwLock::new(ClientState::Init(StateWaiter::new()))
        }))
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn local_device(&self) -> Device {
        self.0.local_device.read().unwrap().clone()
    }

    fn net_listener(&self) -> &NetListener {
        &self.0.net_listener
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
                    state: active
                } => {
                    let waiter = waiter.transfer();
                    let sessions = if let Some(session) = active.cur_session() {
                        vec![session]
                    } else {
                        vec![]
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

    pub fn index(&self) -> usize {
        self.0.sn_index
    }


    async fn update_local(&self, local: Endpoint, outer: Endpoint) {
        info!("{} update local {} => {}", self, local, outer);
        let update = self.net_listener().update_outer(&local, &outer);
        if update > UpdateOuterResult::None {
            let mut local = self.local_device();
            let device_sn_list = local.mut_connect_info().mut_sn_list();
            device_sn_list.clear();
            device_sn_list.push(self.sn().clone());

            let device_endpoints = local.mut_connect_info().mut_endpoints();
            device_endpoints.clear();
            let bound_endpoints = self.net_listener().endpoints();
            for ep in bound_endpoints {
                device_endpoints.push(ep);
            }

            let stack = Stack::from(&self.0.stack);
            let _ = sign_and_set_named_object_body(
                stack.keystore().signer(),
                &mut local,
                &SignatureSource::RefIndex(0),
            ).await;

            let updated = {
                let mut store = self.0.local_device.write().unwrap();
                if store.body().as_ref().unwrap().update_time() < local.body().as_ref().unwrap().update_time() {
                    *store = local;
                    true
                } else {
                    false
                }
            };

            if updated {
                self.ping_once();
            }
        }
    }

    fn ping_once(&self) {
        info!("{} ping once", self);
        let mut state = self.0.state.write().unwrap();
        match &mut *state {
            ClientState::Active { 
                state: active, 
                .. 
            } => {
                match active {
                    ActiveState::Wait(_, session) => {
                        let session = session.reset(Some(self.local_device()), None);
                        *active = ActiveState::FirstTry(session.clone_as_ping_session());
                        {
                        
                            let client = self.clone();
                            let session = session.clone_as_ping_session();
                            task::spawn(async move {
                                client.sync_session_resp(session.as_ref(), session.wait().await);
                            });
                        }
                    }, 
                    _ => {}
                }
            },
            _ => {}
        }
    }

    fn sync_session_resp(&self, session: &dyn PingSession, result: BuckyResult<PingSessionResp>) {
        info!("{} wait session {} finished {:?}", self, session, result);
        struct NextStep {
            waiter: Option<StateWaiter>, 
            update: Option<(Endpoint, Endpoint)>, 
            to_start: Option<Box<dyn PingSession>>, 
            ping_once: bool, 
            update_cache: Option<Option<Endpoint>>
        }

        impl NextStep {
            fn none() -> Self {
                Self {
                    waiter: None, 
                    update: None, 
                    to_start: None, 
                    ping_once: false, 
                    update_cache: None
                }
            }
        }

        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                ClientState::Connecting {
                    waiter, 
                    sessions 
                } => {
                    if let Some(index) = sessions.iter().enumerate().find_map(|(index, exists)| if exists.local() == session.local() { Some(index) } else { None }) {
                        match result {
                            Ok(resp) => {
                                let mut next = NextStep::none();
                                next.waiter = Some(waiter.transfer());

                                if resp.endpoints.len() > 0 {
                                    next.update = Some((session.local(), resp.endpoints[0]));
                                }

                                info!("{} online", self);

                                next.update_cache = Some(Some(resp.from));

                                *state = ClientState::Active {
                                    waiter: StateWaiter::new(), 
                                    state: ActiveState::Wait(bucky_time_now() + self.0.config.interval.as_micros() as u64, session.reset(None, Some(resp.from)))
                                };
                                
                                next
                            }, 
                            Err(_err) => {
                                sessions.remove(index);
                                let mut next = NextStep::none();
                                if sessions.len() == 0 {
                                    error!("{} timeout", self);
                                    next.waiter = Some(waiter.transfer());
                                    *state = ClientState::Timeout;
                                }

                                next
                            }
                        }
                    } else {
                        NextStep::none()
                    }
                }, 
                ClientState::Active { 
                    waiter, 
                    state: active 
                } => {
                    if active.cur_session().and_then(|exists| if exists.local() == session.local() { Some(()) } else { None }).is_some() {
                        match result {
                            Ok(resp) => {
                                *active = ActiveState::Wait(bucky_time_now() + self.0.config.interval.as_micros() as u64, session.reset(None, None));
                                
                                let mut next = NextStep::none();
                                if resp.endpoints.len() > 0 {
                                    next.update = Some((session.local(), resp.endpoints[0]));
                                } else if resp.err == BuckyErrorCode::NotFound {
                                    next.ping_once = true;
                                }
                                next
                            },
                            Err(_err) => {
                                match active {
                                    ActiveState::FirstTry(session) => {
                                        let stack = Stack::from(&self.0.stack);
                                        stack.keystore().reset_peer(&self.sn());
                                        let session = session.reset(None, None);
                                        info!("{} start second try", self);
                                        *active = ActiveState::SecondTry(session);
                                        NextStep::none()
                                    }, 
                                    ActiveState::SecondTry(_) => {
                                        let mut next = NextStep::none();
                                        next.waiter = Some(waiter.transfer());
                                        error!("{} timeout", self);
                                        *state = ClientState::Timeout;
                                        next.update_cache = Some(None);
                                        next
                                    },
                                    _ => NextStep::none()
                                }
                            }
                        }
                    } else {
                        NextStep::none()
                    }
                }, 
                _ => NextStep::none()
            }
        };

        if let Some(update) = next.update_cache {
            let stack = Stack::from(&self.0.stack);
            if let Some(remote) = update {
                stack.sn_client().cache().add_active(session.sn(), EndpointPair::from((session.local().clone(), remote)));
            } else {
                stack.sn_client().cache().remove_active(session.sn());
            }
        }

        if let Some(waiter) = next.waiter {
            waiter.wake();
        }

        if let Some(session) = next.to_start {
            let client = self.clone();
            task::spawn(async move {
                client.sync_session_resp(session.as_ref(), session.wait().await);
            });
        }

        if let Some((local, outer)) = next.update {
            let client = self.clone();
            task::spawn(async move {
                client.update_local(local, outer).await;
            });
        } else if next.ping_once {
            self.ping_once();
        }

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
                _ => NextStep::Return(Err(BuckyError::new(BuckyErrorCode::ErrorState, "not online"))), 
            }
        };
       
        match next {
            NextStep::Return(result) => result, 
            NextStep::Wait(waiter) => {
                StateWaiter::wait(waiter, || {
                    let state = self.0.state.read().unwrap();
                    match &*state {
                        ClientState::Stopped => Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled")), 
                        ClientState::Timeout =>  Ok(()), 
                        _ => unreachable!()
                    }
                }).await
            }
        }
    }

    pub async fn wait_online(&self) -> BuckyResult<SnStatus> {
        info!("{} waiting online", self);
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
                ClientState::Active {..} => NextStep::Return(Ok(SnStatus::Online)), 
                ClientState::Timeout =>  NextStep::Return(Ok(SnStatus::Offline)), 
                ClientState::Stopped => NextStep::Return(Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"))), 
            }
        };
       
        let state = || {
            let state = self.0.state.read().unwrap();
            match &*state {
                ClientState::Active {..} => Ok(SnStatus::Online), 
                ClientState::Timeout =>  Ok(SnStatus::Offline), 
                ClientState::Stopped => Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled")), 
                _ => unreachable!()
            }
        };

        match next {
            NextStep::Return(result) => result, 
            NextStep::Wait(waiter) => StateWaiter::wait(waiter, state).await, 
            NextStep::Start(waiter) => {
                info!("{} started", self);
                let mut sessions = vec![];
                for local in self.0.net_listener.udp().iter().filter(|interface| interface.local().addr().is_ipv4()) {
                    let sn_endpoints: Vec<Endpoint> = self.0.sn.connect_info().endpoints().iter().filter(|endpoint| endpoint.is_udp() && endpoint.is_same_ip_version(&local.local())).cloned().collect();
                    if sn_endpoints.len() > 0 {
                        let params = UdpSesssionParams {
                            config: self.0.config.udp.clone(), 
                            local: local.clone(),
                            local_device: self.local_device(), 
                            with_device: true, 
                            sn_desc: self.0.sn.desc().clone(),
                            sn_endpoints,  
                        };
                        let session = UdpPingSession::new(self.0.stack.clone(), self.0.gen_seq.clone(), params).clone_as_ping_session();
                        info!("{} add session {}", self, session);
                        sessions.push(session);
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
                            client.sync_session_resp(session.as_ref(), session.wait().await);
                        });
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
                    state: active, 
                    .. 
                } => {
                    match active {
                        ActiveState::Wait(next_time, session) => {
                            if now > *next_time {
                                let session = session.clone_as_ping_session();
                                *active = ActiveState::FirstTry(session.clone_as_ping_session());
                                {
                                
                                    let client = self.clone();
                                    let session = session.clone_as_ping_session();
                                    task::spawn(async move {
                                        client.sync_session_resp(session.as_ref(), session.wait().await);
                                    });
                                }
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
                    state: active, 
                    .. 
                } => {
                    if let Some(session) = active.cur_session().and_then(|session| if session.local() == interface.local() { Some(session) } else { None }) {
                        vec![session]
                    } else {
                        vec![]
                    }
                }, 
                _ => vec![]
            }
        };

        for session in sessions {
            let _ = session.on_udp_ping_resp(resp, from);
        }
    }


}




