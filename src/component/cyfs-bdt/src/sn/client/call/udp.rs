use std::{
    sync::{Arc, RwLock}, 
    time::{Duration}, 
};
use futures::future::AbortRegistration;
use cyfs_base::*;
use crate::{
    types::*,
    interface::{udp::{Interface, PackageBoxEncodeContext}}, 
    protocol::{v0::*}, 
};
use super::client::{WeakSession, CallTunnel};

#[derive(Clone)]
pub struct Config {
    pub resend_interval: Duration
}

enum UdpState {
    Init(StateWaiter), 
    Running {
        waiter: StateWaiter, 
        first_send_time: Timestamp,  
        last_send_time: Timestamp
    }, 
    Responsed {
        active: EndpointPair, 
        result: BuckyResult<Device>
    }, 
    Canceled(BuckyError)
}

struct UdpImpl {
    owner: WeakSession, 
    locals: Vec<Interface>, 
    remotes: Vec<Endpoint>, 
    state: RwLock<UdpState>
}

#[derive(Clone)]
pub(super) struct UdpCall(Arc<UdpImpl>);

impl std::fmt::Display for UdpCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let session = self.0.owner.to_strong();
        write!(f, "UdpCall{{owner:{:?}, locals:{:?}, remotes:{:?}}}", session, self.0.locals, self.0.remotes)
    }
}

impl UdpCall {
    pub fn new(owner: WeakSession, locals: Vec<Interface>, remotes: Vec<Endpoint>) -> Self {
        Self(Arc::new(UdpImpl {
            owner, 
            locals, 
            remotes, 
            state: RwLock::new(UdpState::Init(StateWaiter::new()))
        }))
    } 

    fn send_call(&self) {
        struct SendIter {
            tunnel: UdpCall, 
            local_index: usize, 
            remote_index: usize
        }

        impl SendIter {
            fn new(tunnel: UdpCall) -> Self {
                Self {
                    tunnel, 
                    local_index: 0, 
                    remote_index: 0
                }
            }
        }

        impl Iterator for SendIter {
            type Item = (Interface, Endpoint);

            fn next(&mut self) -> Option<Self::Item> {
                if self.local_index == self.tunnel.0.locals.len() {
                    return None;
                }
                if self.remote_index == self.tunnel.0.remotes.len() {
                    self.remote_index = 0;
                    self.local_index += 1;
                    if self.local_index == self.tunnel.0.locals.len() {
                        return None;
                    } 
                }
                let ret = (self.tunnel.0.locals[self.local_index].clone(), self.tunnel.0.remotes[self.remote_index].clone());
                self.remote_index += 1;
                Some(ret)
            }
        }

        if let Some(session) = self.0.owner.to_strong() {
            let mut context = PackageBoxEncodeContext::default();
            let _ = Interface::send_box_mult(&mut context, session.packages().as_ref(), SendIter::new(self.clone()), |_, _, _| true);
        }
    }
}

#[async_trait::async_trait]
impl CallTunnel for UdpCall {
    fn clone_as_call_tunnel(&self) -> Box<dyn CallTunnel> {
        Box::new(self.clone())
    }

    async fn wait(&self) -> (BuckyResult<Device>, Option<EndpointPair>) {
        enum NextStep {
            Start(AbortRegistration), 
            Wait(AbortRegistration),
            Return((BuckyResult<Device>, Option<EndpointPair>))
        }

        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                UdpState::Init(waiter) => {
                    let next = NextStep::Start(waiter.new_waiter());
                    let now = bucky_time_now();
                    *state = UdpState::Running {
                        waiter: waiter.transfer(), 
                        first_send_time: now, 
                        last_send_time: now
                    };
                    next
                }, 
                UdpState::Running {
                    waiter, 
                    ..
                } => NextStep::Wait(waiter.new_waiter()), 
                UdpState::Responsed { 
                    active, 
                    result 
                } => NextStep::Return((result.clone(), Some(active.clone()))), 
                UdpState::Canceled(err) => NextStep::Return((Err(err.clone()), None))
            }
        };

        let state = || {
            let state = self.0.state.read().unwrap();
            match &*state {
                UdpState::Responsed { 
                    active, 
                    result 
                } => (result.clone(), Some(active.clone())), 
                UdpState::Canceled(err) => (Err(err.clone()), None),
                _ => unreachable!()
            }
        };

        match next {
            NextStep::Start(waiter) => {
                self.send_call();
                StateWaiter::wait(waiter, state).await
            }, 
            NextStep::Wait(waiter) => StateWaiter::wait(waiter, state).await,
            NextStep::Return(ret) => ret
        }
    }

    fn on_time_escape(&self, now: Timestamp) {
        enum NextStep {
            None, 
            Send, 
            Wake(StateWaiter)
        }

        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                UdpState::Running { 
                    first_send_time, 
                    last_send_time, 
                    waiter
                } => {
                    if let Some(session) = self.0.owner.to_strong() {
                        if now > *first_send_time && Duration::from_micros(now - *first_send_time) > session.config().timeout {
                            let next = NextStep::Wake(waiter.transfer());
                            *state = UdpState::Canceled(BuckyError::new(BuckyErrorCode::Timeout, "udp call timeout"));
                            next
                        } else if now > *last_send_time && Duration::from_micros(now - *last_send_time) > session.config().udp.resend_interval {
                            *last_send_time = now;
                            NextStep::Send
                        } else {
                            NextStep::None
                        }
                    } else {
                        let next = NextStep::Wake(waiter.transfer());
                        *state = UdpState::Canceled(BuckyError::new(BuckyErrorCode::ErrorState, "udp call canceled"));
                        next
                    }
                }, 
                _ => NextStep::None
            }
        };

        match next {
            NextStep::Send => {
                self.send_call();
            }, 
            NextStep::Wake(waiter) => {
                waiter.wake();
            },
            NextStep::None => {}
        }
    }

    fn cancel(&self) {
        let waiter = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                UdpState::Running {
                    waiter, 
                    ..
                } => {
                    let waiter = waiter.transfer();
                    *state = UdpState::Canceled(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"));
                    Some(waiter)
                }, 
                _ => None
            }
        };
        
        if let Some(waiter) = waiter {
            waiter.wake();
        }
    }

    fn on_udp_call_resp(&self, resp: &SnCallResp, local: &Interface, from: &Endpoint) {
        let waiter = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                UdpState::Running {
                    waiter, 
                    ..
                } => {
                    let waiter = waiter.transfer();
                    *state = UdpState::Responsed { 
                        active: EndpointPair::from((local.local(), *from)), 
                        result: if let Some(device) = resp.to_peer_info.clone() {
                            Ok(device)
                        } else {
                            Err(BuckyError::new(BuckyErrorCode::from(resp.result as u16), "sn response error"))
                        }
                    };
                    Some(waiter)
                }, 
                _ => None
            }
        };
        
        if let Some(waiter) = waiter {
            waiter.wake();
        }
    }
}