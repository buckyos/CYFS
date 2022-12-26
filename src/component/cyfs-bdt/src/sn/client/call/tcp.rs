use std::{
    sync::{Arc, RwLock}, 
    time::{Duration}, 
};
use async_std::{
    task
};
use futures::future::{Abortable, AbortHandle, AbortRegistration};
use cyfs_base::*;
use crate::{
    types::*,
    interface::{tcp::{Interface}}, 
    protocol::{*, v0::*}, 
};
use super::client::{WeakSession, CallTunnel};

enum TcpState {
    Init(StateWaiter), 
    Running { 
        proc_stub: AbortHandle, 
        waiter: StateWaiter, 
    }, 
    Responsed {
        result: BuckyResult<Device>
    }, 
    Canceled(BuckyError)
}

struct TcpImpl {
    owner: WeakSession, 
    timeout: Duration,  
    remote: Endpoint, 
    state: RwLock<TcpState>
}

#[derive(Clone)]
pub(super) struct TcpCall(Arc<TcpImpl>);

impl TcpCall {
    pub fn new(owner: WeakSession, timeout: Duration, remote: Endpoint) -> Self {
        Self(Arc::new(TcpImpl {
            owner, 
            timeout, 
            remote, 
            state: RwLock::new(TcpState::Init(StateWaiter::new()))
        }))
    } 

    async fn call_proc(&self) -> BuckyResult<BuckyResult<Device>> {
        let session = self.0.owner.to_strong()
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"))?;
        let stack = session.stack();
        let packages = session.packages();

        let start = bucky_time_now();
        let interface = Interface::connect(
            self.0.remote.clone(), 
            packages.remote().clone(), 
            stack.device_cache().get_inner(packages.remote()).unwrap().desc().clone(), 
            packages.key().clone(), 
            self.0.timeout).await?;
        
        let escaped = Duration::from_micros(bucky_time_now() - start);

        if self.0.timeout <= escaped {
            return Err(BuckyError::new(BuckyErrorCode::Timeout, ""))
        }
        let sn_call: &SnCall = packages.packages_no_exchange()[0].as_ref();
        let resp_box = interface.confirm_connect(&stack, vec![DynamicPackage::from(sn_call.clone())], self.0.timeout - escaped).await?;
        if resp_box.packages_no_exchange().len() > 0 {
            let resp_pkg = &resp_box.packages_no_exchange()[0];
            if resp_pkg.cmd_code() == PackageCmdCode::SnCallResp {
                let call_resp: &SnCallResp = resp_pkg.as_ref();
                Ok(if let Some(device) = call_resp.to_peer_info.clone() {
                    Ok(device)
                } else {
                    Err(BuckyError::new(BuckyErrorCode::from(call_resp.result as u16), "sn response error"))
                })
            } else {
                Err(BuckyError::new(BuckyErrorCode::Failed, "invalid resp box type"))
            }
        } else {
            Err(BuckyError::new(BuckyErrorCode::Failed, "empty resp box"))
        }
    }

    async fn call_once(&self, abort: AbortRegistration) {
        if let Ok(result) = Abortable::new(self.call_proc(), abort).await {
            let waiter = {
                let mut state = self.0.state.write().unwrap();
                match &mut *state {
                    TcpState::Running {
                        waiter, 
                        ..
                    } => {
                        let waiter = waiter.transfer(); 
                        *state = match result {
                            Ok(result) => TcpState::Responsed { result }, 
                            Err(err) => TcpState::Canceled(err) 
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
}

#[async_trait::async_trait]
impl CallTunnel for TcpCall {
    fn clone_as_call_tunnel(&self) -> Box<dyn CallTunnel> {
        Box::new(self.clone())
    }

    async fn wait(&self) -> (BuckyResult<Device>, Option<EndpointPair>) {
        enum NextStep {
            Start {
                waiter: AbortRegistration, 
                abort: AbortRegistration
            }, 
            Wait(AbortRegistration),
            Return((BuckyResult<Device>, Option<EndpointPair>))
        }

        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                TcpState::Init(waiter) => {
                    let (proc_stub, reg) = AbortHandle::new_pair();
                    let next = NextStep::Start {
                        waiter: waiter.new_waiter(), 
                        abort: reg
                    };
                    *state = TcpState::Running {
                        waiter: waiter.transfer(), 
                        proc_stub
                    };
                    next
                }, 
                TcpState::Running {
                    waiter, 
                    ..
                } => NextStep::Wait(waiter.new_waiter()), 
                TcpState::Responsed { 
                    result 
                } => NextStep::Return((result.clone(), Some(EndpointPair::from((Endpoint::default_tcp(&self.0.remote), self.0.remote.clone()))))), 
                TcpState::Canceled(err) => NextStep::Return((Err(err.clone()), None))
            }
        };

        let state = || {
            let state = self.0.state.read().unwrap();
            match &*state {
                TcpState::Responsed { 
                    result 
                } => (result.clone(), Some(EndpointPair::from((Endpoint::default_tcp(&self.0.remote), self.0.remote.clone())))), 
                TcpState::Canceled(err) => (Err(err.clone()), None),
                _ => unreachable!()
            }
        };

        match next {
            NextStep::Start {
                waiter, 
                abort 
            } => {
                let tunnel = self.clone();
                task::spawn(async move {
                    tunnel.call_once(abort).await;
                });
                StateWaiter::wait(waiter, state).await
            }, 
            NextStep::Wait(waiter) => StateWaiter::wait(waiter, state).await,
            NextStep::Return(ret) => ret
        }
    }

    fn reset(&self, timeout: Duration) -> Option<Box<dyn CallTunnel>> {
        Some(Box::new(Self(Arc::new(TcpImpl {
            owner: self.0.owner.clone(), 
            timeout, 
            remote: self.0.remote.clone(), 
            state: RwLock::new(TcpState::Init(StateWaiter::new()))
        }))))
    }

    fn cancel(&self) {
        let stub = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                TcpState::Running {
                    waiter, 
                    proc_stub, 
                    ..
                } => {
                    let waiter = waiter.transfer();
                    let abort = proc_stub.clone();
                    *state = TcpState::Canceled(BuckyError::new(BuckyErrorCode::Interrupted, "user canceled"));
                    Some((waiter, abort))
                }, 
                _ => None
            }
        };
        
        if let Some((waiter, stub)) = stub {
            waiter.wake();
            stub.abort();
        }
    }
}


