use log::*;
use std::{
    sync::RwLock,
};
use async_std::{sync::Arc, task};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::*, 
    stream::{StreamContainer, StreamProviderSelector}, 
    tunnel::{self, Tunnel, ProxyType}, 
    stack::{Stack, WeakStack}
};
use super::super::{action::*};
use super::{action::*};

enum ConnectTcpStreamState {
    Connecting1(StateWaiter), 
    PreEstablish(tcp::Interface), 
    Connecting2, 
    Establish, 
    Closed
}

struct ConnectTcpStreamImpl {
    stack: WeakStack, 
    tunnel: tunnel::tcp::Tunnel, 
    stream: StreamContainer, 
    state: RwLock<ConnectTcpStreamState>,
}

impl std::fmt::Display for ConnectTcpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConnectTcpStream{{stream:{},local:{},remote:{}}}", self.0.stream, self.local(), self.remote())
    }
}

#[derive(Clone)]
pub struct ConnectTcpStream(Arc<ConnectTcpStreamImpl>);

impl ConnectTcpStream {
    pub fn new(
        stack: WeakStack, 
        stream: StreamContainer, 
        tunnel: tunnel::tcp::Tunnel) -> Self {
        let a = Self(Arc::new(ConnectTcpStreamImpl {
            stack, 
            stream: stream.clone(),
            tunnel,   
            state: RwLock::new(ConnectTcpStreamState::Connecting1(StateWaiter::new()))
        }));

        {
            // 同步 stream 的establish状态
            // 当stream 的wait establish 返回时，action要么已经进入establish状态了，要么中止所有动作进入closed状态
            // 如果已经进入PreEstablish 状态了， 但是选择了其他的action 进入continue connect；已经联通的tcp interface交给对应的TcpTunnel来 active
            let ca = a.clone();
            let stream = stream.clone();
            task::spawn(async move {
                let (opt_waiter, tunnel_interface) = match stream.wait_establish().await {
                    Ok(_) => {
                        let state = &mut *ca.0.state.write().unwrap();
                        match state {
                            ConnectTcpStreamState::Connecting1(ref mut waiter) => {
                                let waiter = Some(waiter.transfer());
                                *state = ConnectTcpStreamState::Closed;
                                (waiter, None)
                            }, 
                            ConnectTcpStreamState::PreEstablish(interface) => {
                                let interface = interface.clone();
                                *state = ConnectTcpStreamState::Closed;
                                (None, Some(interface))
                            }, 
                            ConnectTcpStreamState::Establish => {
                                // do nothing
                                (None, None)
                            }, 
                            _ => {
                                *state = ConnectTcpStreamState::Closed;
                                (None, None)
                            }
                        }
                    },
                    Err(_) => {
                        let state = &mut *ca.0.state.write().unwrap();
                        let ret = match state {
                            ConnectTcpStreamState::Connecting1(ref mut waiter) => {
                                let waiter = Some(waiter.transfer());
                                (waiter, None)
                            }, 
                            ConnectTcpStreamState::PreEstablish(interface) => {
                                (None, Some(interface.clone()))
                            }, 
                            _ => {
                                (None, None)
                            }
                        };
                        *state = ConnectTcpStreamState::Closed;
                        ret
                    }
                };
                
                if let Some(waiter) = opt_waiter {
                    waiter.wake()
                }

                if let Some(interface) = tunnel_interface {
                    let _ = ca.0.tunnel.connect_with_interface(interface);
                }
            });
        }

        {
            // 发起tcp 连接，tcp 连上时，进入pre establish
            // tcp连不上， 直接进入 closed 状态
            let ca = a.clone();
            task::spawn(async move {
                if let Some(tunnel) = ca.0.stream.tunnel() {
                    let keystore = tunnel.stack().keystore().clone();
                    let key = keystore.create_key(tunnel.remote_const(), false);
                    let connect_result = tcp::Interface::connect(/*ca.local().addr().ip(),*/
                                                                        *ca.remote(),
                                                                        tunnel.remote().clone(),
                                                                        tunnel.remote_const().clone(),
                                                                        key.key, 
                                                                        Stack::from(&ca.0.stack).config().tunnel.tcp.connect_timeout
                    ).await;

                    
                    let opt_waiter = match connect_result {
                        Ok(interface) => {
                            let state = &mut *ca.0.state.write().unwrap();
                            match state {
                                ConnectTcpStreamState::Connecting1(waiter) => {
                                    debug!("{} Connecting1=>PreEstablish", ca);
                                    let waiter = Some(waiter.transfer());
                                    *state = ConnectTcpStreamState::PreEstablish(interface);
                                    waiter
                                }, 
                                _ => {
                                    None
                                }
                            }
                        }, 
                        Err(_) => {
                            ca.0.tunnel.mark_dead(ca.0.tunnel.state());
                            let state = &mut *ca.0.state.write().unwrap();
                            match state {
                                ConnectTcpStreamState::Connecting1(waiter) => {
                                    debug!("{} Connecting1=>Closed", ca);
                                    let waiter = Some(waiter.transfer());
                                    *state = ConnectTcpStreamState::Closed;
                                    waiter
                                }, 
                                _ => {
                                    *state = ConnectTcpStreamState::Closed;
                                    None
                                }
                            }
                        }
                    };

                    if let Some(waiter) = opt_waiter {
                        waiter.wake()
                    }
                }
            });
        }
        
        a
    }
}

impl BuildTunnelAction for ConnectTcpStream {
    fn local(&self) -> &Endpoint {
        &self.0.tunnel.local()
    }

    fn remote(&self) -> &Endpoint {
        &self.0.tunnel.remote()
    }
}

#[async_trait]
impl ConnectStreamAction for ConnectTcpStream {
    fn clone_as_connect_stream_action(&self) -> DynConnectStreamAction {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn state(&self) -> ConnectStreamState {
        match &*self.0.state.read().unwrap() {
            ConnectTcpStreamState::Connecting1(_) => ConnectStreamState::Connecting1, 
            ConnectTcpStreamState::PreEstablish(_) => ConnectStreamState::PreEstablish,
            ConnectTcpStreamState::Connecting2 => ConnectStreamState::Connecting2,
            ConnectTcpStreamState::Establish => ConnectStreamState::Establish,
            ConnectTcpStreamState::Closed => ConnectStreamState::Closed,
        }
    }
    async fn wait_pre_establish(&self) -> ConnectStreamState {
        let (state, opt_waiter) = match &mut *self.0.state.write().unwrap() {
            ConnectTcpStreamState::Connecting1(ref mut waiter) => {
                (ConnectStreamState::Connecting1, Some(waiter.new_waiter()))
            }, 
            ConnectTcpStreamState::PreEstablish(_) => (ConnectStreamState::PreEstablish, None),
            ConnectTcpStreamState::Connecting2 => (ConnectStreamState::Connecting2, None),
            ConnectTcpStreamState::Establish => (ConnectStreamState::Establish, None),
            ConnectTcpStreamState::Closed => (ConnectStreamState::Closed, None),
        };
        if let Some(waiter) = opt_waiter {
            StateWaiter::wait(waiter, | | self.state()).await
        } else {
            state
        }
    }

    async fn continue_connect(&self) -> BuckyResult<StreamProviderSelector> {
        // 向已经联通的tcp interface发 tcp syn connection，收到对端返回的tcp ack connection时establish
        let interface = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                ConnectTcpStreamState::PreEstablish(interface) => {
                    debug!("{} PreEstablish=>Connecting2", self);
                    let interface = interface.clone();
                    *state = ConnectTcpStreamState::Connecting2;
                    Ok(interface)
                },
                _ => {
                    let err = BuckyError::new(BuckyErrorCode::ErrorState, "continue connect on tcp stream not pre establish");
                    debug!("{} continue_connect failed for err", self);
                    Err(err)
                }
            }
        }?;
        let syn_stream = self.0.stream.syn_tcp_stream().ok_or_else(|| BuckyError::new(BuckyErrorCode::ErrorState, "continue connect on stream not connecting"))?;
        let ack = match interface.confirm_connect(&Stack::from(&self.0.stack), vec![DynamicPackage::from(syn_stream)], Stack::from(&self.0.stack).config().tunnel.tcp.confirm_timeout).await {
            Ok(resp_box) => {
                let packages = resp_box.packages_no_exchange();
                if packages.len() == 1 && packages[0].cmd_code() == PackageCmdCode::TcpAckConnection {
                    let ack: &TcpAckConnection = packages[0].as_ref();
                    //FIXME: 处理TcpAckConnection中的字段
                    // TcpStream 可以联通的时候，让对应的TcpTunnel进入pre active 状态
                    let _ = self.0.tunnel.pre_active(ack.to_device_desc.body().as_ref().unwrap().update_time());
                    let state = &mut *self.0.state.write().unwrap();
                    match state {
                        ConnectTcpStreamState::Connecting2 => {
                            *state = ConnectTcpStreamState::Establish;
                            Ok(ack.clone())
                        }, 
                        _ => Err(BuckyError::new(BuckyErrorCode::ErrorState, "tcp stram got ack but action not in connecting2 state"))
                    }
                } else {
                    let state = &mut *self.0.state.write().unwrap();
                    *state = ConnectTcpStreamState::Closed;
                    Err(BuckyError::new(BuckyErrorCode::InvalidInput, "tcp stream got error confirm when expecting TcpAckConnection"))
                }
            }, 
            Err(e) => {
                let state = &mut *self.0.state.write().unwrap();
                *state = ConnectTcpStreamState::Closed;
                Err(e)
            }
        }.map_err(|e| {
            // self.0.tunnel.mark_dead(self.0.tunnel.state());
            e
        })?;

        Ok(StreamProviderSelector::Tcp(
                interface.socket().clone(), 
                interface.key().clone(), 
                Some(ack.clone())))
    }
}


enum AcceptReverseTcpStreamState {
    Connecting1(StateWaiter), 
    PreEstablish(tcp::AcceptInterface), 
    Connecting2, 
    Establish, 
    Closed
}

struct AcceptReverseTcpStreamImpl {
    local: Endpoint, 
    remote: Endpoint,
    stream: StreamContainer, 
    state: RwLock<AcceptReverseTcpStreamState>
}

impl std::fmt::Display for AcceptReverseTcpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AcceptReverseTcpStream{{stream:{},local:{},remote:{}}}", self.0.stream, self.0.local, self.0.remote)
    }
}

#[derive(Clone)]
pub struct AcceptReverseTcpStream(Arc<AcceptReverseTcpStreamImpl>);

impl AcceptReverseTcpStream {
    pub fn new(stream: StreamContainer, local: Endpoint, remote: Endpoint) -> Self {
        let a = Self(Arc::new(AcceptReverseTcpStreamImpl {
            local, 
            remote, 
            stream: stream.clone(),  
            state: RwLock::new(AcceptReverseTcpStreamState::Connecting1(StateWaiter::new()))
        }));

        {
            // 同步 stream 的establish状态
            // 当stream 的wait establish 返回时，action要么已经进入establish状态了，要么中止所有动作进入closed状态
            let ca = a.clone();
            let stream = stream.clone();
            task::spawn(async move {
                let (waiter, interface) = match stream.wait_establish().await {
                    Ok(_) => {
                        let state = &mut *ca.0.state.write().unwrap();
                        match state {
                            AcceptReverseTcpStreamState::Connecting1(ref mut waiter) => {
                                let waiter = Some(waiter.transfer());
                                *state = AcceptReverseTcpStreamState::Closed;
                                (waiter, None)
                            }, 
                            AcceptReverseTcpStreamState::PreEstablish(interface) => {
                                (None, Some(interface.clone()))
                            }, 
                            AcceptReverseTcpStreamState::Establish => {
                                // do nothing
                                (None, None)
                            }, 
                            _ => {
                                *state = AcceptReverseTcpStreamState::Closed;
                                (None, None)
                            }
                        }
                    },
                    Err(_) => {
                        let state = &mut *ca.0.state.write().unwrap();
                        match state {
                            AcceptReverseTcpStreamState::Connecting1(ref mut waiter) => {
                                let waiter = Some(waiter.transfer());
                                *state = AcceptReverseTcpStreamState::Closed;
                                (waiter, None)
                            }, 
                            AcceptReverseTcpStreamState::PreEstablish(interface) => {
                                (None, Some(interface.clone()))
                            }, 
                            _ => {
                                *state = AcceptReverseTcpStreamState::Closed;
                                (None, None)
                            }
                        }
                    }
                };
                
                if let Some(waiter) = waiter {
                    waiter.wake()
                }

                if let Some(interface) = interface {
                    let ack_ack_stream = stream.ack_ack_tcp_stream(TCP_ACK_CONNECTION_RESULT_REFUSED);
                    let _ = match interface.confirm_accept(vec![DynamicPackage::from(ack_ack_stream)]).await {
                        Ok(_) => {
                            debug!("{} confirm {} with refuse tcp connection ", stream, interface);
                        }, 
                        Err(e) => {
                            warn!("{} confirm {} with tcp ack ack connection failed for {}", stream, interface, e);
                            if let Some(tunnel) = stream.tunnel() {
                                let tunnel = tunnel.create_tunnel::<tunnel::tcp::Tunnel>(EndpointPair::from((*interface.local(), Endpoint::default_tcp(interface.local()))), ProxyType::None);
                                if let Ok((tunnel, _)) = tunnel {
                                    tunnel.mark_dead(tunnel.state());
                                }   
                            }
                        }
                    };
                }
            });
        }


        a
    }
}


impl BuildTunnelAction for AcceptReverseTcpStream {
    fn local(&self) -> &Endpoint {
        &self.0.local
    }

    fn remote(&self) -> &Endpoint {
        &self.0.remote
    }
}

#[async_trait]
impl ConnectStreamAction for AcceptReverseTcpStream {
    fn clone_as_connect_stream_action(&self) -> DynConnectStreamAction {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn state(&self) -> ConnectStreamState {
        match &*self.0.state.read().unwrap() {
            AcceptReverseTcpStreamState::Connecting1(_) => ConnectStreamState::Connecting1, 
            AcceptReverseTcpStreamState::PreEstablish(_) => ConnectStreamState::PreEstablish,
            AcceptReverseTcpStreamState::Connecting2 => ConnectStreamState::Connecting2,
            AcceptReverseTcpStreamState::Establish => ConnectStreamState::Establish,
            AcceptReverseTcpStreamState::Closed => ConnectStreamState::Closed,
        }
    }

    async fn wait_pre_establish(&self) -> ConnectStreamState {
        let (state, opt_waiter) = match &mut *self.0.state.write().unwrap() {
            AcceptReverseTcpStreamState::Connecting1(ref mut waiter) => {
                (ConnectStreamState::Connecting1, Some(waiter.new_waiter()))
            }, 
            AcceptReverseTcpStreamState::PreEstablish(_) => (ConnectStreamState::PreEstablish, None),
            AcceptReverseTcpStreamState::Connecting2 => (ConnectStreamState::Connecting2, None),
            AcceptReverseTcpStreamState::Establish => (ConnectStreamState::Establish, None),
            AcceptReverseTcpStreamState::Closed => (ConnectStreamState::Closed, None),
        };
        if let Some(waiter) = opt_waiter {
            StateWaiter::wait(waiter, | | self.state()).await
        } else {
            state
        }
    }

    async fn continue_connect(&self) -> BuckyResult<StreamProviderSelector> {
        // 向已经联通的tcp interface发 tcp syn connection，收到对端返回的tcp ack connection时establish
        let interface = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                AcceptReverseTcpStreamState::PreEstablish(interface) => {
                    let interface = interface.clone();
                    *state = AcceptReverseTcpStreamState::Connecting2;
                    Ok(interface)
                },
                _ => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "continue connect on tcp stream not pre establish"))
                }
            }
        }?;
        let ack_ack_stream = self.0.stream.ack_ack_tcp_stream(TCP_ACK_CONNECTION_RESULT_OK);
        let _ = match interface.confirm_accept(vec![DynamicPackage::from(ack_ack_stream)]).await {
            Ok(_) => {
                let state = &mut *self.0.state.write().unwrap();
                match state {
                    AcceptReverseTcpStreamState::Connecting2 => {
                        *state = AcceptReverseTcpStreamState::Establish;
                        Ok(())
                    }, 
                    _ => Err(BuckyError::new(BuckyErrorCode::ErrorState, "tcp stram got ack but action not in connecting2 state"))
                }
            }, 
            Err(e) => {
                let state = &mut *self.0.state.write().unwrap();
                *state = AcceptReverseTcpStreamState::Closed;
                Err(e)
            }
        }?;

        Ok(StreamProviderSelector::Tcp(
                interface.socket().clone(), 
                interface.key().clone(), 
                None))
    }
}

impl OnPackage<TcpAckConnection, tcp::AcceptInterface> for AcceptReverseTcpStream {
    fn on_package(&self, _pkg: &TcpAckConnection, interface: tcp::AcceptInterface) -> Result<OnPackageResult, BuckyError> {
        // 在 connecting1 状态下accept 到 带着 TcpAckConnection 的 tcp stream， 进入pre establish
        let waiter = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                AcceptReverseTcpStreamState::Connecting1(ref mut waiter) => {
                    debug!("{} Connecting1=>PreEstablish", self);
                    let waiter = waiter.transfer();
                    *state = AcceptReverseTcpStreamState::PreEstablish(interface);
                    Ok(waiter)
                }, 
                _ => {
                    debug!("{} ingnore tcp ack connection for not in connecting1", self);
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "not in connecting1"))
                }
            }
        }?;
        waiter.wake();
        Ok(OnPackageResult::Handled)
    }
}

impl From<DynConnectStreamAction> for AcceptReverseTcpStream {
    fn from(action: DynConnectStreamAction) -> Self {
        action.as_any().downcast_ref::<Self>().unwrap().clone()
    }
} 