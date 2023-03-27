use log::*;
use std::{
    sync::{Arc, RwLock,}, 
};
use async_std::{
    task
};
use futures::future::{AbortRegistration};
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
    client::*
};

enum ClientsState {
    Init(StateWaiter), 
    Connecting {
        waiter: StateWaiter, 
        client: PingClient
    }, 
    Active {
        waiter: StateWaiter, 
        client: PingClient
    }, 
    Offline, 
    Stopped,
}

struct StateImpl { 
    remain: Vec<(usize, DeviceId)>, 
    state: ClientsState
}

struct ClientsImpl {
    stack: WeakStack, 
    net_listener: NetListener, 
    sn_list: Vec<Device>, 
    local_device: Device, 
    gen_seq: Arc<TempSeqGenerator>, 
    state: RwLock<StateImpl>
}

#[derive(Clone)]
pub struct PingClients(Arc<ClientsImpl>);

impl std::fmt::Display for PingClients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stack = Stack::from(&self.0.stack);
        write!(f, "PingClients{{local:{}}}", stack.local_device_id())
    }
}

impl PingClients {
    pub(crate) fn reset(
        &self, 
        net_listener: NetListener, 
        local_device: Device
    ) -> Self {
        info!("{} reset", self);

        let remain = {
            let state = self.0.state.read().unwrap();

            let client = match &state.state {
                ClientsState::Connecting {
                    client, 
                    ..
                } => Some(client.clone()),
                ClientsState::Active {
                    client,
                    ..
                } => Some(client.clone()),
                _ => None
            };

            let mut remain = state.remain.clone();
            
            if let Some(client) = client {
                remain.push((client.index(), client.sn().clone()));
            } 
            
            remain
        };

        Self(Arc::new(ClientsImpl {
            stack: self.0.stack.clone(), 
            gen_seq: self.0.gen_seq.clone(), 
            net_listener, 
            local_device, 
            sn_list: self.0.sn_list.clone(),  
            state: RwLock::new(StateImpl {
                remain, 
                state: ClientsState::Init(StateWaiter::new())
            })
        }))
    }

    pub(crate) fn new(
        stack: WeakStack, 
        gen_seq: Arc<TempSeqGenerator>, 
        net_listener: NetListener, 
        sn_list: Vec<Device>, 
        local_device: Device
    ) -> Self {
        let strong_stack = Stack::from(&stack);
        let mut remain: Vec<(usize, DeviceId)> = sn_list.iter().map(|d| d.desc().device_id()).enumerate().collect();
        remain.sort_by(|(_, l), (_, r)| r.object_id().distance(strong_stack.local_device_id().object_id()).cmp(&l.object_id().distance(strong_stack.local_device_id().object_id())));
   
        Self(Arc::new(ClientsImpl {
            stack, 
            gen_seq, 
            net_listener, 
            local_device, 
            sn_list,  
            state: RwLock::new(StateImpl {
                remain, 
                state: ClientsState::Init(StateWaiter::new())
            })
        }))
    }

    pub fn net_listener(&self) -> &NetListener {
        &self.0.net_listener
    }

    pub fn default_local(&self) -> Device {
        if let Some(client) = self.default_client() {
            client.local_device()
        } else {
            self.0.local_device.clone()
        }
        
    }

    pub fn default_client(&self) -> Option<PingClient> {
        let state = self.0.state.read().unwrap();
        match &state.state {
            ClientsState::Connecting { client, .. } => Some(client.clone()), 
            ClientsState::Active { client, .. } => Some(client.clone()), 
            _ => None
        }
    }

    pub fn status(&self) -> Option<SnStatus> {
        let state = self.0.state.read().unwrap();
        match &state.state {
            ClientsState::Active { .. } => Some(SnStatus::Online), 
            ClientsState::Offline => Some(SnStatus::Offline), 
            _ => None
        }
    }

    fn sync_ping_client(&self, client: &PingClient, result: BuckyResult<SnStatus>) {
        info!("{} client {} finished {:?}", self, client, result);
        if result.is_err() {
            self.stop();
            return ;
        }
        struct NextStep {
            waiter: Option<StateWaiter>, 
            to_start: Option<PingClient>, 
            to_wait: Option<PingClient>
        }

        impl NextStep {
            fn none() -> Self {
                Self {
                    waiter: None, 
                    to_start: None, 
                    to_wait: None
                }
            }
        }
        let status = result.unwrap();
        let next = {
            let mut state = self.0.state.write().unwrap();

            let next_index = state.remain.last().map(|(index, _)| *index);
            
            let next = match &mut state.state {
                ClientsState::Connecting {
                    waiter, 
                    client: connecting
                } => {
                    let mut next = NextStep::none();
                    if client.ptr_eq(connecting) {
                        match status {
                            SnStatus::Online => {
                                next.waiter = Some(waiter.transfer());
                                info!("{} online with client {}", self, client);
                                state.state = ClientsState::Active {
                                    waiter: StateWaiter::new(), 
                                    client: client.clone()
                                };
                                next.to_wait = Some(client.clone());       
                            },
                            SnStatus::Offline => {
                                if let Some(index) = next_index {
                                    let stack = Stack::from(&self.0.stack);
                                    let client = PingClient::new(
                                        self.0.stack.clone(), 
                                        stack.config().sn_client.ping.clone() , 
                                        self.0.gen_seq.clone(), 
                                        self.0.net_listener.reset(None), 
                                        index, 
                                        self.0.sn_list[index].clone(), 
                                        self.0.local_device.clone());
                                    next.to_start = Some(client.clone());
                                    *connecting = client;
                                } else {
                                    next.waiter = Some(waiter.transfer());
                                    error!("{} offline", self);
                                    state.state = ClientsState::Offline;
                                }
                            }
                        }
                    }
                    next
                }, 
                ClientsState::Active {
                    waiter, 
                    client: active
                } => {
                    let mut next = NextStep::none();
                    if client.ptr_eq(active) {
                        match status {
                            SnStatus::Online => {
                                next.waiter = Some(waiter.transfer());
                                info!("{} online with client {}", self, client);
                                state.state = ClientsState::Active {
                                    waiter: StateWaiter::new(), 
                                    client: client.clone()
                                };
                                next.to_wait = Some(client.clone());       
                            },
                            SnStatus::Offline => {
                                if let Some(index) = next_index {
                                    let stack = Stack::from(&self.0.stack);
                                    let client = PingClient::new(
                                        self.0.stack.clone(), 
                                        stack.config().sn_client.ping.clone() , 
                                        self.0.gen_seq.clone(), 
                                        self.0.net_listener.reset(None), 
                                        index, 
                                        self.0.sn_list[index].clone(), 
                                        self.0.local_device.clone());
                                    next.to_start = Some(client.clone());
                                    *active = client;
                                } else {
                                    next.waiter = Some(waiter.transfer());
                                    state.state = ClientsState::Offline;
                                }
                            }
                        }
                    }
                    next
                }, 
                _ => NextStep::none(),
            };
            if next.to_wait.is_some() || next.to_start.is_some() {
                let _ = state.remain.pop();
            }

            next
        };
        
        if let Some(waiter) = next.waiter {
            waiter.wake();
        }

        if let Some(client) = next.to_start {
            let clients = self.clone();
            task::spawn(async move {
                info!("{} start next client {}", clients, client);
                clients.sync_ping_client(&client, client.wait_online().await);
            });
        }

        if let Some(client) = next.to_wait {
            let clients = self.clone();
            task::spawn(async move {
                clients.sync_ping_client(&client, client.wait_offline().await.map(|_| SnStatus::Offline));
            });
        }
    } 

    pub async fn wait_online(&self) -> BuckyResult<SnStatus> {
        log::info!("{} wait online", self);
        enum NextStep {
            Wait(AbortRegistration), 
            Start(AbortRegistration, PingClient), 
            Wake(StateWaiter, BuckyResult<SnStatus>), 
            Return(BuckyResult<SnStatus>)
        }
        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.state {
                ClientsState::Init(waiter) => {
                    let mut waiter = waiter.transfer();
                    let stack = Stack::from(&self.0.stack);
                    if let Some((index, _)) = state.remain.pop() {
                        let client = PingClient::new(
                            self.0.stack.clone(), 
                            stack.config().sn_client.ping.clone() , 
                            self.0.gen_seq.clone(), 
                            self.0.net_listener.reset(None), 
                            index, 
                            self.0.sn_list[index].clone(), 
                            self.0.local_device.clone());
                        let next = NextStep::Start(waiter.new_waiter(), client.clone());
                        state.state = ClientsState::Connecting { waiter, client };
                        next
                    } else {
                        let waiter = waiter.transfer();
                        let result = Err(BuckyError::new(BuckyErrorCode::Interrupted, "empty sn list"));
                        state.state = ClientsState::Stopped;
                        NextStep::Wake(waiter, result)   
                    }
                }, 
                ClientsState::Connecting {
                    waiter, 
                    ..
                } => NextStep::Wait(waiter.new_waiter()), 
                ClientsState::Active { .. } => NextStep::Return(Ok(SnStatus::Online)), 
                ClientsState::Offline => NextStep::Return(Ok(SnStatus::Offline)), 
                ClientsState::Stopped => NextStep::Return(Err(BuckyError::new(BuckyErrorCode::Interrupted, "empty sn list")))
            } 
        };


        let state = || {
            let state = self.0.state.read().unwrap();
            match &state.state {
                ClientsState::Active { .. } => Ok(SnStatus::Online), 
                ClientsState::Offline => Ok(SnStatus::Offline), 
                ClientsState::Stopped => Err(BuckyError::new(BuckyErrorCode::Interrupted, "empty sn list")), 
                _ => unreachable!()
            }
        };
        
        match next {
            NextStep::Return(result) => result, 
            NextStep::Wake(waiter, result) => {
                waiter.wake();
                result 
            },
            NextStep::Wait(waiter) => StateWaiter::wait(waiter, state).await, 
            NextStep::Start(waiter, client) => {
                let clients = self.clone();
                task::spawn(async move {
                    clients.sync_ping_client(&client, client.wait_online().await);
                });
                StateWaiter::wait(waiter, state).await
            }
        }
    }

    pub async fn wait_offline(&self) -> BuckyResult<()> {
        info!("{} wait offline", self);
        enum NextStep {
            Wait(AbortRegistration),
            Return(BuckyResult<()>)
        }

        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.state {
                ClientsState::Stopped => NextStep::Return(Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"))), 
                ClientsState::Active {
                    waiter, 
                    ..
                } => NextStep::Wait(waiter.new_waiter()), 
                ClientsState::Offline =>  NextStep::Return(Ok(())), 
                _ => NextStep::Return(Err(BuckyError::new(BuckyErrorCode::ErrorState, "not online")))
            }
        };
       
        match next {
            NextStep::Return(result) => result, 
            NextStep::Wait(waiter) => {
                StateWaiter::wait(waiter, || {
                    let state = self.0.state.read().unwrap();
                    match &state.state {
                        ClientsState::Stopped => Err(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled")), 
                        ClientsState::Offline =>  Ok(()), 
                        _ => unreachable!()
                    }
                }).await
            }
        }
    }

    pub fn stop(&self) {
        info!("{} stop", self);
        let (waiter, client) = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.state {
                ClientsState::Init(waiter) => {
                    let waiter = waiter.transfer();
                    state.state = ClientsState::Stopped;
                    (Some(waiter), None)
                }, 
                ClientsState::Connecting {
                    waiter, 
                    client
                } => {
                    let waiter = waiter.transfer();
                    let client = client.clone();
                    state.state = ClientsState::Stopped;
                    (Some(waiter), Some(client))
                },
                ClientsState::Active {
                    waiter, 
                    client
                } => {
                    let waiter = waiter.transfer();
                    let client = client.clone();
                    state.state = ClientsState::Stopped;
                    (Some(waiter), Some(client))
                },
                _ => (None, None)
            }
        };

        if let Some(waiter) = waiter {
            waiter.wake()
        };

        if let Some(client) = client {
            client.stop();
        }
    }

    pub fn sn_list(&self) -> &Vec<Device> {
        &self.0.sn_list
    }

    pub fn on_time_escape(&self, now: Timestamp) {
        let client = {
            let state = self.0.state.read().unwrap();
            match &state.state {
                ClientsState::Connecting {
                    client, 
                    ..
                } => Some(client.clone()), 
                ClientsState::Active {
                    client,
                    ..
                } => Some(client.clone()), 
                _ => None, 
            }
        };

        if let Some(client) = client {
            client.on_time_escape(now);
        }
    }

    pub fn on_udp_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, interface: Interface) {
        log::info!("{} ping-resp, sn: {}/{}, seq: {}.", self, resp.sn_peer_id.to_string(), from.to_string(), resp.seq.value());

        let client = {
            let state = self.0.state.read().unwrap();
            match &state.state {
                ClientsState::Connecting {
                    client, 
                    ..
                } => {
                    if resp.sn_peer_id.eq(client.sn()) {
                        Some(client.clone())
                    } else {
                        None
                    }
                }, 
                ClientsState::Active {
                    client,
                    ..
                } => {
                    if resp.sn_peer_id.eq(client.sn()) {
                        Some(client.clone())
                    } else {
                        None
                    }
                }, 
                _ => None, 
            }
        };

        if let Some(client) = client {
            client.on_udp_ping_resp(resp, &from, interface);
        } else {
            warn!("{} ping-resp, sn: {}/{} not found, maybe is stopped.", self, resp.sn_peer_id.to_string(), from.to_string());
        }
    }

    pub fn on_called(&self, called: &SnCalled, in_box: &PackageBox, from: &Endpoint, from_interface: Interface) {
        info!("{} called, called: {:?}", self, called);
        let stack = Stack::from(&self.0.stack);

        if !called.to_peer_id.eq(stack.local_device_id()) {
            warn!("{} called, recv called to other: {}.", self, called.to_peer_id);
            return;
        }
        let client = {
            let state = self.0.state.read().unwrap();
            match &state.state {
                ClientsState::Active {
                    client,
                    ..
                } => {
                    if called.sn_peer_id.eq(client.sn()) {
                        Some(client.clone())
                    } else {
                        None
                    }
                }, 
                _ => None, 
            }
        };

        if client.is_some() {
            let resp = SnCalledResp {
                seq: called.seq,
                result: 0,
                sn_peer_id: called.sn_peer_id.clone(),
            };
    
            let mut pkg_box = PackageBox::encrypt_box(resp.sn_peer_id.clone(), in_box.key().clone());
            pkg_box.push(resp);
    
            let mut context = PackageBoxEncodeContext::default();
            let _ = from_interface.send_box_to(&mut context, &pkg_box, from);
    
            let _ = stack.on_called(&called, ());
    
        } else {
            warn!("{} the sn maybe is removed when recv called-req. from {}", self, called.to_peer_id);
        }   
    }
}




