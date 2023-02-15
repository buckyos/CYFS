use std::{
    //time::Duration,
    sync::RwLock, 
};
use async_std::{sync::{Arc}, future, task};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*},  
    stream::{StreamContainer, StreamProviderSelector}
};
use super::super::{action::*};
use super::{action::*};
use log::*;

enum ConnectPackageStreamState {
    Connecting1(StateWaiter), 
    PreEstablish(SessionData), 
    Connecting2, 
    Establish, 
    Closed
}

impl std::fmt::Display for ConnectPackageStreamState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConnectPackageStreamState::Connecting1(_) => write!(f, "Connecting1"),
            ConnectPackageStreamState::PreEstablish(_) => write!(f, "PreEstablish"),
            ConnectPackageStreamState::Connecting2 => write!(f, "Connecting2"),
            ConnectPackageStreamState::Establish => write!(f, "Establish"),
            ConnectPackageStreamState::Closed => write!(f, "Closed"),
        }
    }
}

struct ConnectPackageStreamImpl {
    local: Endpoint, 
    remote: Endpoint,
    stream: StreamContainer, 
    state: RwLock<ConnectPackageStreamState>,
}   

#[derive(Clone)]
pub struct ConnectPackageStream(Arc<ConnectPackageStreamImpl>);

impl std::fmt::Display for ConnectPackageStream {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ConnectPackageStream {{stream:{}}}", self.0.stream)
    }
}

impl ConnectPackageStream {
    pub fn endpoint_pair() -> EndpointPair {
        EndpointPair::from((Endpoint::default(), Endpoint::default()))
    }

    pub fn new(stream: StreamContainer) -> Self {
        let a = Self(Arc::new(ConnectPackageStreamImpl {
            local: Endpoint::default(), 
            remote: Endpoint::default(),
            stream: stream.clone(),  
            state: RwLock::new(ConnectPackageStreamState::Connecting1(StateWaiter::new()))
        }));

        {
            // 同步 stream 的establish状态
            // 当stream 的wait establish 返回时，action要么已经进入establish状态了，要么中止所有动作进入closed状态
            let ca = a.clone();
            let stream = stream.clone();
            task::spawn(async move {
                let opt_waiter = match stream.wait_establish().await {
                    Ok(_) => {
                        let state = &mut *ca.0.state.write().unwrap();
                        match state {
                            ConnectPackageStreamState::Connecting1(ref mut waiter) => {
                                let waiter = Some(waiter.transfer());
                                *state = ConnectPackageStreamState::Closed;
                                waiter
                            }, 
                            ConnectPackageStreamState::Establish => {
                                // do nothing
                                None
                            }, 
                            _ => {
                                *state = ConnectPackageStreamState::Closed;
                                None
                            }
                        }
                    },
                    Err(_) => {
                        let state = &mut *ca.0.state.write().unwrap();
                        match state {
                            ConnectPackageStreamState::Connecting1(ref mut waiter) => {
                                let waiter = Some(waiter.transfer());
                                *state = ConnectPackageStreamState::Closed;
                                waiter
                            }, 
                            _ => {
                                *state = ConnectPackageStreamState::Closed;
                                None
                            }
                        }
                    }
                };
                
                if let Some(waiter) = opt_waiter {
                    waiter.wake()
                }
            });
        }
        a
    }

    pub fn begin(&self) {
        // 只要还处于 connecting1 状态， 不断重发syn session data
        let ca = self.clone();
        let syn_session_data = ca.0.stream.syn_session_data();
        if let Some(syn_session_data) = syn_session_data {
            let resend_interval = ca.0.stream.stack().config().stream.stream.package.connect_resend_interval;
            task::spawn(async move {
                //在进入pre establish之前，重发 syn session data
                loop {
                    match ca.state() {
                        ConnectStreamState::Connecting1 => {
                            trace!("{} send sync session data", ca);
                            if let Some(tunnel) = ca.0.stream.tunnel() {
                                let _ = tunnel.send_packages(vec![DynamicPackage::from(syn_session_data.clone_with_data())]);
                            } else {
                                break;
                            }
                        }, 
                        _ => break
                    };
                    future::timeout(resend_interval, future::pending::<()>()).await.err();
                }
            });
        } else {
            debug!("{} ingore sync sync session data proc for stream not in connecting state", self);
        }
    }
}

#[async_trait]
impl BuildTunnelAction for ConnectPackageStream {
    fn local(&self) -> &Endpoint {
        &self.0.local
    }

    fn remote(&self) -> &Endpoint {
        &self.0.remote
    }
}

#[async_trait]
impl ConnectStreamAction for ConnectPackageStream {
    fn clone_as_connect_stream_action(&self) -> DynConnectStreamAction {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn state(&self) -> ConnectStreamState {
        match &*self.0.state.read().unwrap() {
            ConnectPackageStreamState::Connecting1(_) => ConnectStreamState::Connecting1, 
            ConnectPackageStreamState::PreEstablish(_) => ConnectStreamState::PreEstablish,
            ConnectPackageStreamState::Connecting2 => ConnectStreamState::Connecting2,
            ConnectPackageStreamState::Establish => ConnectStreamState::Establish,
            ConnectPackageStreamState::Closed => ConnectStreamState::Closed,
        }
    }
    async fn wait_pre_establish(&self) -> ConnectStreamState {
        let (state, opt_waiter) = match &mut *self.0.state.write().unwrap() {
            ConnectPackageStreamState::Connecting1(ref mut waiter) => {
                (ConnectStreamState::Connecting1, Some(waiter.new_waiter()))
            }, 
            ConnectPackageStreamState::PreEstablish(_) => (ConnectStreamState::PreEstablish, None),
            ConnectPackageStreamState::Connecting2 => (ConnectStreamState::Connecting2, None),
            ConnectPackageStreamState::Establish => (ConnectStreamState::Establish, None),
            ConnectPackageStreamState::Closed => (ConnectStreamState::Closed, None),
        };
        if let Some(waiter) = opt_waiter {
            StateWaiter::wait(waiter, | | self.state()).await
        } else {
            state
        }
    }

    async fn continue_connect(&self) -> BuckyResult<StreamProviderSelector> {
        // 让 package stream 联通， 发送不带 syn flag的session data包的逻辑应该在 package stream provider中完成 
        let sesstion_data = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                ConnectPackageStreamState::PreEstablish(syn_ack) => {
                    info!("{} PreEstablish=>Establish", self);
                    let sesstion_data = syn_ack.clone_with_data();
                    *state = ConnectPackageStreamState::Establish;
                    Ok(sesstion_data)
                }, 
                _ => {
                    error!("{} continue connect failed for in state {}", self, state);
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "continue connect on package stream not pre establish"))
                }
            }
        }?;

        let remote_id = sesstion_data.syn_info.clone().unwrap().from_session_id;

        Ok(StreamProviderSelector::Package(remote_id, Some(sesstion_data)))
    }
}

impl OnPackage<SessionData> for ConnectPackageStream {
    fn on_package(&self, pkg: &SessionData, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        if pkg.is_syn() {
            unreachable!()
        } else if pkg.is_syn_ack() {
            // 在 connecting1 状态下收到 syn ack session data， 进入pre establish
            let opt_waiter = {
                let state = &mut *self.0.state.write().unwrap();
                match state {
                    ConnectPackageStreamState::Connecting1(ref mut waiter) => {
                        info!("{} Connecting1=>PreEstablish", self);
                        let waiter = Some(waiter.transfer());
                        *state = ConnectPackageStreamState::PreEstablish(pkg.clone_with_data());
                        waiter
                    }, 
                    _ => {None}
                }
            };
            if let Some(waiter) = opt_waiter {
                waiter.wake();
            }
            Ok(OnPackageResult::Handled)
        } else {
            unreachable!()
        }
    }
}

impl From<DynConnectStreamAction> for ConnectPackageStream {
    fn from(action: DynConnectStreamAction) -> Self {
        action.as_any().downcast_ref::<Self>().unwrap().clone()
    }
} 