use log::*;
use async_std::{
    future, 
    task, 
    sync::{Arc}
};
use std::{
    time::Duration, 
    fmt, 
    sync::{RwLock}, 
    collections::BTreeMap, 
    ops::Deref,
    convert::TryFrom
};
use cyfs_base::*;
use crate::{
    types::*,
    protocol::{*, v0::*},
    interface::{
        self, 
        udp::{
            OnUdpPackageBox, 
            OnUdpRawData
        },
        tcp::{
            OnTcpInterface
        }
    },
    sn::client::PingClientCalledEvent, 
    stream::{StreamContainer, RemoteSequence}, 
    stack::{Stack, WeakStack},
    MTU
};
use super::{
    tunnel::*, 
    builder::*, 
    udp, 
    tcp
};

#[derive(Clone)]
pub struct BuildTunnelParams {
    pub remote_const: DeviceDesc, 
    pub remote_sn: Vec<DeviceId>, 
    pub remote_desc: Option<Device>,
}

#[derive(Clone)]
pub struct Config {
    pub retain_timeout: Duration,  
    pub connect_timeout: Duration, 
    pub tcp: tcp::Config, 
    pub udp: udp::Config
}

enum TunnelBuildState {
    Idle, 
    ConnectStream(ConnectStreamBuilder), 
    AcceptStream(AcceptStreamBuilder), 
    ConnectTunnel(ConnectTunnelBuilder), 
    AcceptTunnel(AcceptTunnelBuilder)
}

pub enum StreamConnectorSelector {
    Package(Timestamp), 
    Tcp(tcp::Tunnel, Timestamp),
    Builder(ConnectStreamBuilder)
}

struct TunnelDeadState {
    //标记从哪个状态进入的dead
    former_state: TunnelState, 
    //什么时候进入dead
    when: Timestamp
}

struct TunnelConnectingState {
    waiter: StateWaiter, 
    build_state: TunnelBuildState, 
}

struct TunnelActiveState {
    remote_timestamp: Timestamp, 
    default_tunnel: DynamicTunnel
}

// 这里为了逻辑简单，dead状态之后不能回退；
enum TunnelStateImpl {
    Connecting(TunnelConnectingState), 
    Active(TunnelActiveState),
    Dead(TunnelDeadState)
}

enum TunnelRecycleState {
    InUse, 
    Recycle(Timestamp)
}

struct TunnelContainerState {
    last_update: Timestamp, 
    recyle_state: TunnelRecycleState, 
    tunnel_state: TunnelStateImpl, 
    tunnel_entries: BTreeMap<EndpointPair, DynamicTunnel>
}

struct TunnelContainerImpl {
    stack: WeakStack,
    config: Config, 
    remote: DeviceId,  
    remote_const: DeviceDesc, 
    sequence_generator: TempSeqGenerator, 
    state: RwLock<TunnelContainerState>,
}

#[derive(Clone)]
pub struct TunnelContainer(Arc<TunnelContainerImpl>);

impl TunnelContainer {
    pub(super) fn new(stack: WeakStack, remote_const: DeviceDesc, config: Config) -> Self {
        Self(Arc::new(TunnelContainerImpl {
            stack, 
            config, 
            remote: remote_const.device_id(), 
            remote_const, 
            sequence_generator: TempSeqGenerator::new(), 
            state: RwLock::new(TunnelContainerState {
                recyle_state: TunnelRecycleState::InUse, 
                tunnel_entries: BTreeMap::new(), 
                last_update: bucky_time_now(), 
                tunnel_state: TunnelStateImpl::Connecting(TunnelConnectingState {
                    waiter: StateWaiter::new(), 
                    build_state: TunnelBuildState::Idle
                })
            }), 
        }))
    }

    pub fn mtu(&self) -> usize {
        if let Ok(tunnel) = self.default_tunnel() {
            tunnel.mtu()
        } else {
            MTU-12
        }
    }

    fn mark_in_use(&self) {
        let mut state = self.0.state.write().unwrap();
        state.recyle_state = TunnelRecycleState::InUse;
    }

    fn mark_recycle(&self, when: Timestamp) -> Timestamp {
        let mut state = self.0.state.write().unwrap();
        match &state.recyle_state {
            TunnelRecycleState::InUse => {
                state.recyle_state = TunnelRecycleState::Recycle(when);
                when
            }, 
            TunnelRecycleState::Recycle(when) => *when
        }
    }

    fn sync_connecting(&self) {
        let connect_timeout = self.config().connect_timeout;
        let tunnel = self.clone();
        task::spawn(async move {
            match future::timeout(connect_timeout, tunnel.wait_active()).await {
                Ok(_state) => {
                    // do thing
                }, 
                Err(_err) => {
                    let waiter = {
                        let state = &mut *tunnel.0.state.write().unwrap();
                        match &mut state.tunnel_state {
                            TunnelStateImpl::Connecting(connecting) => {
                                let mut ret_waiter = StateWaiter::new();
                                connecting.waiter.transfer_into(&mut ret_waiter);
                                state.last_update = bucky_time_now();
                                state.tunnel_state = TunnelStateImpl::Dead(TunnelDeadState {
                                    former_state: TunnelState::Connecting, 
                                    when: bucky_time_now()
                                });
                                state.tunnel_entries.clear();
                                Some(ret_waiter)
                            }, 
                            _ => {
                                None
                            }
                        }
                    };
                    if let Some(waiter) = waiter {
                        info!("{} dead for connect timeout", tunnel);
                        waiter.wake();
                    }
                }
            }
        });
    }

    pub fn config(&self) -> &Config {
        &self.0.config
    }

    pub fn stack(&self) -> Stack {
        Stack::from(&self.0.stack)
    }

    pub fn remote(&self) -> &DeviceId {
        &self.0.remote
    }

    pub fn remote_const(&self) -> &DeviceDesc {
        &self.0.remote_const
    }

    pub fn protocol_version(&self) -> u8 {
        0
    }

    pub fn stack_version(&self) -> u32 {
        0
    }

    pub fn default_tunnel(&self) -> BuckyResult<DynamicTunnel> {
        let state = self.0.state.read().unwrap();
        match &state.tunnel_state {
            TunnelStateImpl::Active(active) => {
                Ok(active.default_tunnel.clone())
            }, 
            TunnelStateImpl::Connecting(_) => {
                let entries = &state.tunnel_entries;
                let mut iter = entries.iter();
                loop {
                    match iter.next() {
                        Some((ep_pair, tunnel)) => {
                            if let TunnelState::Active(_) = tunnel.as_ref().state() {
                                if ep_pair.protocol() == Protocol::Udp {
                                    break Some(tunnel.clone());
                                } 
                            }
                        },
                        None => break None
                    }
                }.ok_or_else(| | BuckyError::new(BuckyErrorCode::NotFound, "no default tunnel"))
            },
            TunnelStateImpl::Dead(_) => {
                Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's dead"))
            }
        }
    }

    pub fn default_udp_tunnel(&self) -> BuckyResult<udp::Tunnel> {
        let tunnel = self.default_tunnel()?;
        if tunnel.as_ref().local().is_udp() {
            Ok(tunnel.clone_as_tunnel())
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, "default tunnel not udp"))
        }
    }

    pub fn send_packages(&self, packages: Vec<DynamicPackage>) -> Result<(), BuckyError> {
        let tunnel = self.default_tunnel()?;
        for package in packages {
            tunnel.as_ref().send_package(package)?;
        }
        Ok(())
    }

    pub fn send_package(&self, package: DynamicPackage) -> Result<(), BuckyError> {
        let tunnel = self.default_tunnel()?;
        tunnel.as_ref().send_package(package)
    }

    pub fn send_plaintext(&self, package: DynamicPackage) -> Result<(), BuckyError> {
        let tunnel = self.default_tunnel()?;

        let mut buf = vec![0u8; MTU];

        let buf_len = buf.len();
        let enc_from = tunnel.as_ref().raw_data_header_len();

        let mut context = merge_context::FirstEncode::new();
        let enc: &dyn RawEncodeWithContext<merge_context::FirstEncode> = package.as_ref();
        let buf_ptr = enc.raw_encode_with_context(&mut buf[enc_from..], &mut context, &None)?;

        let len = buf_len - buf_ptr.len();

        match tunnel.as_ref().send_raw_data(&mut buf[..len]) {
            Ok(_) => Ok(()),
            Err(e) => Err(BuckyError::new(BuckyErrorCode::Failed, format!("{}", e)))
        }
    }

    pub fn build_send(&self, package: DynamicPackage, build_params: BuildTunnelParams, plaintext: bool) -> BuckyResult<()> {
        let (tunnel, builder) = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.tunnel_state {
                TunnelStateImpl::Active(active) => {
                    (Some(active.default_tunnel.clone()), None)
                }, 
                TunnelStateImpl::Connecting(connecting) => {
                    (None, match connecting.build_state {
                        TunnelBuildState::Idle => {
                            // 创建新的 tunnel builder
                            let builder = ConnectTunnelBuilder::new(self.0.stack.clone(), self.clone(), build_params);
                            connecting.build_state = TunnelBuildState::ConnectTunnel(builder.clone());
                            Some(builder)
                        }, 
                        _ => {
                            // do nothing
                            None
                        }
                    })
                },
                TunnelStateImpl::Dead(_) => {
                    let builder = ConnectTunnelBuilder::new(self.0.stack.clone(), self.clone(), build_params);
                    state.last_update = bucky_time_now();
                    state.tunnel_state = TunnelStateImpl::Connecting(TunnelConnectingState {
                        waiter: StateWaiter::new(), 
                        build_state: TunnelBuildState::ConnectTunnel(builder.clone())
                    });
                    (None, Some(builder))
                }
            }
        };

        if let Some(tunnel) = tunnel {
            trace!("{} send packages from {}", self, tunnel.as_ref().as_ref());
            if plaintext {
                self.send_plaintext(package)
            } else {
                tunnel.as_ref().send_package(package)
            }
        } else if let Some(builder) = builder {
            //FIXME: 加入到connecting的 send 缓存里面去  
            self.stack().keystore().reset_peer(self.remote());
            self.sync_connecting();          
            task::spawn(async move {
                builder.build().await;
            });
            Ok(())
        } else {
            //FIXME: 加入到connecting的 send 缓存里面去  
            Ok(())
        }
    }

    pub fn state(&self) -> TunnelState {
        match &self.0.state.read().unwrap().tunnel_state {
            TunnelStateImpl::Connecting(_) => TunnelState::Connecting, 
            TunnelStateImpl::Active(active) => TunnelState::Active(active.remote_timestamp), 
            TunnelStateImpl::Dead(_) => TunnelState::Dead
        }
    }

    pub async fn wait_active(&self) -> TunnelState {
        let (state, waiter) = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.tunnel_state {
                TunnelStateImpl::Connecting(connecting) => {
                    (TunnelState::Connecting, Some(connecting.waiter.new_waiter()))
                },
                TunnelStateImpl::Active(active) => {
                    (TunnelState::Active(active.remote_timestamp), None)
                },
                TunnelStateImpl::Dead(_) => {
                    (TunnelState::Dead, None)
                }
            }
        };
        if let Some(waiter) = waiter {
            StateWaiter::wait(waiter, | | self.state()).await
        } else {
            state
        }
    }

    pub fn tunnel_of<T: 'static + Tunnel + Clone>(&self, ep_pair: &EndpointPair) -> Option<T> {
        let tunnel_impl = &self.0;
        let entries = &tunnel_impl.state.read().unwrap().tunnel_entries;
        entries.get(ep_pair).map(|tunnel| tunnel.clone_as_tunnel())
    }

    pub fn create_tunnel<T: 'static + Tunnel + Clone>(
        &self, 
        ep_pair: EndpointPair, 
        proxy: ProxyType) -> Result<T, BuckyError> {
        trace!("{} try create tunnel on {}", self, ep_pair);
        let stack = self.stack();
        if stack.net_manager().listener().ep_set().get(ep_pair.remote()).is_some() {
            trace!("{} ignore creat tunnel on {} for remote is self", self, ep_pair);
            return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "remote is self"));
        }

        let tunnel_impl = &self.0;
        let (tunnel, _newly_create) = {
            let entries = &mut tunnel_impl.state.write().unwrap().tunnel_entries;
            if let Some(tunnel) = entries.get(&ep_pair) {
                //FIXME: 如果是NAT1的情况，存在在收到AckProxy之前，从ProxyEndpoint上收到通过代理转发过来的RN包，
                //      此时udp_tunnel会已经存在，并且ProxyType为None；应当考虑在这这修改udp_tunnel的ProxyType,并且触发syn_tunnel_state
                //      以正确的选择default tunnel
                trace!("{} create tunnel return existing tunnel", self);
                (tunnel.clone(), None)
            } else {
                let dynamic_tunnel = match ep_pair.protocol() {
                    Protocol::Udp => {
                        let stack = Stack::from(&tunnel_impl.stack);
                        let interface = stack.net_manager().listener().udp_of(ep_pair.local()).ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "udp interface not found"))?.clone();
                        DynamicTunnel::new(udp::Tunnel::new(
                            self.clone(), 
                            self.clone_as_tunnel_owner(),  
                            interface, 
                            *ep_pair.remote(), 
                            proxy))
                    },
                    Protocol::Tcp => {
                        DynamicTunnel::new(tcp::Tunnel::new(self.clone(), ep_pair.clone()))
                    },
                    _ => {
                        unreachable!()
                    }
                };
                let tunnel = dynamic_tunnel.clone();
                info!("{} tunnel newly created on {} ", self, ep_pair);
                entries.insert(ep_pair, dynamic_tunnel);
                (tunnel.clone(), Some(tunnel))
            }
        };
        Ok(tunnel.clone_as_tunnel())
        
    }

    pub(crate) fn generate_sequence(&self) -> TempSeq {
        self.0.sequence_generator.generate()
    }

    fn select_stream_connector_by_exists(
        remote_timestamp: Timestamp, 
        tunnel_entries: &BTreeMap<EndpointPair, DynamicTunnel>) -> Option<StreamConnectorSelector> {
        struct Priority {
            tcp: Option<tcp::Tunnel>,
            reverse_tcp: Option<tcp::Tunnel>, 
            package: bool
        }

        let p = {
            let mut priority = Priority {
                tcp: None, 
                reverse_tcp: None, 
                package: false
            };
            for (_, tunnel) in tunnel_entries {
                if let TunnelState::Active(_) = tunnel.as_ref().state() {
                    if tunnel.as_ref().local().is_tcp() {
                        let tunnel = tunnel.clone_as_tunnel::<tcp::Tunnel>();
                        if tunnel.is_reverse() && priority.reverse_tcp.is_none() {
                            priority.reverse_tcp = Some(tunnel);
                        } else {
                            priority.tcp = Some(tunnel);
                            break;
                        }
                    } else {
                        priority.package = true;
                    }
                }
            }

            priority
        };
        
        if p.tcp.is_some() {
            let tunnel = p.tcp.unwrap();
            Some(StreamConnectorSelector::Tcp(tunnel, remote_timestamp))
        } else if p.reverse_tcp.is_some() {
            let tunnel = p.reverse_tcp.unwrap();
            Some(StreamConnectorSelector::Tcp(tunnel, remote_timestamp))
        } else if p.package {
            Some(StreamConnectorSelector::Package(remote_timestamp))
        } else {
            None
        }
    }

    pub(crate) async fn select_stream_connector(
        &self,
        build_params: BuildTunnelParams,  
        stream: StreamContainer) -> BuckyResult<StreamConnectorSelector> {
        let tunnel_impl = &self.0;
        let (selector, new_builder, exists_builder, tunnels) = {
            let mut state = self.0.state.write().unwrap();
            match &mut state.tunnel_state {
                TunnelStateImpl::Active(active) => {
                    let cur_timestamp = active.remote_timestamp;
                    if let Some(selector) = Self::select_stream_connector_by_exists(
                            cur_timestamp, 
                            &state.tunnel_entries) {
                        (Some(selector), None, None, None)
                    } else {
                        error!("{} active but no exists connector", self);
                        let mut tunnel_entries = BTreeMap::new();
                        std::mem::swap(&mut tunnel_entries, &mut state.tunnel_entries);

                        let builder = ConnectStreamBuilder::new(
                            tunnel_impl.stack.clone(), 
                            build_params, 
                            stream);
                        state.last_update = bucky_time_now();
                        state.tunnel_state = TunnelStateImpl::Connecting(TunnelConnectingState {
                            waiter: StateWaiter::new(), 
                            build_state: TunnelBuildState::ConnectStream(builder.clone())
                        });
                        let tunnels: Vec<DynamicTunnel> = tunnel_entries.into_iter().map(|(_, tunnel)| tunnel).collect();
                        (None, Some(builder), None, Some(tunnels))
                    }
                }, 
                TunnelStateImpl::Connecting(connecting) => {
                    match &mut connecting.build_state {
                        TunnelBuildState::Idle => {
                            let builder = ConnectStreamBuilder::new(
                                tunnel_impl.stack.clone(), 
                                build_params, 
                                stream);
                            connecting.build_state = TunnelBuildState::ConnectStream(builder.clone());
                            (None, Some(builder), None, None)
                        }, 
                        TunnelBuildState::ConnectStream(builder) => (None, None, Some(Box::new(builder.clone()) as Box<dyn TunnelBuilder>), None), 
                        TunnelBuildState::AcceptStream(builder) => (None, None, Some(Box::new(builder.clone()) as Box<dyn TunnelBuilder>), None),
                        TunnelBuildState::ConnectTunnel(builder) => (None, None, Some(Box::new(builder.clone()) as Box<dyn TunnelBuilder>), None), 
                        TunnelBuildState::AcceptTunnel(builder) => (None, None, Some(Box::new(builder.clone()) as Box<dyn TunnelBuilder>), None)
                    }
                },
                TunnelStateImpl::Dead(_) => {
                    let builder = ConnectStreamBuilder::new(
                        tunnel_impl.stack.clone(), 
                        build_params, 
                        stream);

                    state.tunnel_state = TunnelStateImpl::Connecting(TunnelConnectingState {
                        waiter: StateWaiter::new(), 
                        build_state: TunnelBuildState::ConnectStream(builder.clone())
                    });

                    (None, Some(builder), None, None)
                }
            }
        };
        if let Some(tunnels) = tunnels {
            for tunnel in tunnels {
                tunnel.as_ref().reset();
            }
        }
        if let Some(selector) = selector {
            Ok(selector)
        } else if let Some(builder) = exists_builder {
            // 如果buidler失败了，都返回错误
            builder.wait_establish().await?;
            let state = self.0.state.read().unwrap();
            match &state.tunnel_state {
                TunnelStateImpl::Active(active) => {
                    Self::select_stream_connector_by_exists(active.remote_timestamp, &state.tunnel_entries)
                        .ok_or_else(|| {
                            error!("{} active but no exists connector", self);
                            BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's dead")
                        })
                }, 
                _ => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's dead"))
                }
            }
        } else if let Some(builder) = new_builder {
            self.stack().keystore().reset_peer(self.remote());
            self.sync_connecting();
            Ok(StreamConnectorSelector::Builder(builder))
        } else {
            unreachable!()
        } 
    } 

    pub(crate) fn payload_size(&self) -> usize {
        1024 // FIXME:先写固定值，一般没连通时需要先ExchangeKey/Syn...，负载会比较小，连通时负载会比较大
    }

    pub fn reset(&self) {
        let (tunnels, waiter) = {
            let mut state = self.0.state.write().unwrap();
            let (waiter, updated) = match &mut state.tunnel_state {
                TunnelStateImpl::Connecting(connecting) => {
                    let mut waiter = StateWaiter::new();
                    connecting.waiter.transfer_into(&mut waiter);
                    state.tunnel_state = TunnelStateImpl::Dead(TunnelDeadState {
                        former_state: TunnelState::Connecting, 
                        when: bucky_time_now()
                    });
                    (Some(waiter), true)
                }, 
                TunnelStateImpl::Active(active) => {
                    state.tunnel_state = TunnelStateImpl::Dead(TunnelDeadState {
                        former_state: TunnelState::Active(active.remote_timestamp), 
                        when: bucky_time_now()
                    });
                    (None, true)
                }, 
                TunnelStateImpl::Dead(_) => {
                    (None, false)
                }
            };
            if updated {
                state.last_update = bucky_time_now();
            }
            let mut tunnel_entries = BTreeMap::new();
            std::mem::swap(&mut tunnel_entries, &mut state.tunnel_entries);
            let tunnels: Vec<DynamicTunnel> = tunnel_entries.into_iter().map(|(_, tunnel)| tunnel).collect();
            (tunnels, waiter)
        };
        for tunnel in tunnels {
            tunnel.as_ref().reset();
        }
        if let Some(waiter) = waiter {
            waiter.wake();
        }
    }

    pub(crate) fn mark_dead(&self, active_timestamp: Timestamp, last_update: Timestamp) -> BuckyResult<()> {
        info!("{} mark dead with active timestamp {} last_update {}", self, active_timestamp, last_update);
        let tunnels: Vec<DynamicTunnel> = {
            let mut state = self.0.state.write().unwrap();
            if state.last_update > last_update {
                info!("{} ignore mark dead for updated", self);
                Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel has updated"))
            } else {
                match &mut state.tunnel_state {
                    TunnelStateImpl::Connecting(_) => {
                        Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's connecting"))
                    }, 
                    TunnelStateImpl::Active(active) => {
                        let cur_timestamp = active.remote_timestamp;
                        if cur_timestamp == active_timestamp {
                            info!("{} Active({})=>Dead", self, cur_timestamp);
                            state.last_update = bucky_time_now();
                            state.tunnel_state = TunnelStateImpl::Dead(TunnelDeadState {
                                former_state: TunnelState::Active(cur_timestamp), 
                                when: bucky_time_now()
                            });
                            let mut tunnel_entries = BTreeMap::new();
                            std::mem::swap(&mut tunnel_entries, &mut state.tunnel_entries);
                            Ok(tunnel_entries.into_iter().map(|(_, tunnel)| tunnel).collect())
                        } else {
                            Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's active"))
                        }
                    }, 
                    TunnelStateImpl::Dead(_) => Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's dead"))
                }
            }
        }?;
        for tunnel in tunnels {
            tunnel.as_ref().reset();
        }
        Ok(())
    }

    pub(super) fn on_raw_data(&self, data: &[u8]) -> BuckyResult<()> {
        let tunnel_impl = &self.0;
        let (cmd_code, buf) = u8::raw_decode(data)?;
        let cmd_code = PackageCmdCode::try_from(cmd_code)?;
        match cmd_code {
            PackageCmdCode::Datagram => {
                let (pkg, _) = Datagram::raw_decode_with_context(buf, &mut merge_context::OtherDecode::default())?;
                let _ = Stack::from(&tunnel_impl.stack).datagram_manager().on_package(&pkg, (self, true));
                Ok(())
            },
            PackageCmdCode::SessionData => unimplemented!(), 
            _ => {
                Stack::from(&tunnel_impl.stack).ndn().channel_manager().on_udp_raw_data(data, self)
            }, 
        }
    }


    
    // 注意R端的打洞包要用SynTunnel不能用AckTunnel
    // 因为可能出现如下时序：L端收到R端的打洞包，停止继续发送打洞包；但是R端没有收到L端的打洞包，继续发送打洞包；
    //   如果R端发的是AckTunnel，L端收到之后不会回复；如果L端改成对AckTunnel回复SynTunnel/AckTunnel都不合适，会导致循环回复
    //   R端发SynTunnel的话，L端收到之后可以回复复AckTunnel 
    // pub(super) fn syn_tunnel_package(&self, syn_tunnel: &SynTunnel, local: Device) -> SynTunnel {
    //     SynTunnel {
    //         protocol_version: self.protocol_version(), 
    //         stack_version: self.stack_version(), 
    //         from_device_id: local.desc().device_id(),
    //         to_device_id: syn_tunnel.from_device_id.clone(),
    //         sequence: syn_tunnel.sequence,
    //         from_device_desc: local,
    //         send_time: 0
    //     }
    // }

    // pub(super) fn ack_tunnel_package(&self, syn_tunnel: &SynTunnel, local: Device) -> AckTunnel {
    //     AckTunnel {
    //         protocol_version: self.protocol_version(), 
    //         stack_version: self.stack_version(), 
    //         sequence: syn_tunnel.sequence,
    //         result: 0,
    //         send_time: 0,
    //         mtu: c::MTU as u16,
    //         to_device_desc: local       
    //     }
    // }
}

impl fmt::Display for TunnelContainer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TunnelContainer{{local:{}, remote:{}}}",
            Stack::from(&self.0.stack).local_device_id(), self.remote())
    }
}


impl TunnelOwner for TunnelContainer {
    fn sync_tunnel_state(&self, tunnel: &DynamicTunnel, former_state: TunnelState, new_state: TunnelState) {
        //TODO: 这里的策略可以调整
        let mut tunnels = vec![];
        let (old, new, waiter) = match new_state {
            TunnelState::Connecting => {
                unreachable!()
            }, 
            TunnelState::Active(remote_timestamp) => {
                let mut state = self.0.state.write().unwrap();
                // 先从entries里面移除
                let entries = &mut state.tunnel_entries;
                let ep_pair = EndpointPair::from((*tunnel.as_ref().local(), *tunnel.as_ref().remote()));
                let exists = {
                    if let Some(stub) = entries.get(&ep_pair) {
                        stub.as_ref().ptr_eq(tunnel) 
                    } else {
                        false
                    }
                };
                let mut to_reset = vec![];

                if exists {
                    for (remote, tunnel) in &state.tunnel_entries {
                        if let TunnelState::Active(active_timestamp) = tunnel.as_ref().state() {
                            if active_timestamp < remote_timestamp {
                                to_reset.push(remote.clone());
                            } 
                        }
                    } 
                    for remote in to_reset {
                        tunnels.push(state.tunnel_entries.remove(&remote).unwrap());
                    }
                    
                    let (ret, updated) = match &mut state.tunnel_state {
                        TunnelStateImpl::Active(active) => {
                            // 如果当前激活的tunnel 属于更新的对端Endpoints
                            let remote_updated = active.remote_timestamp < remote_timestamp;
                            let change_default = remote_updated || {
                                if ProxyType::None != active.default_tunnel.as_ref().proxy() {
                                    // 非代理优先
                                    ProxyType::None == tunnel.as_ref().proxy()
                                } else {
                                    // 单纯的 udp 优先
                                    tunnel.as_ref().local().is_udp() && active.default_tunnel.as_ref().local().is_tcp()
                                    // 主动tcp 优先
                                    // TODO: 简单的主动tcp 优先策略；因为存在如下时序， LN 只有tcp ep 没有udp ep；RN 有tcp ep 和 udp ep； LN 和 RN 在同内网，LN向RN发起stream connect，
                                    //          在特定时序下， LN 和 RN 都会选择被动tcp 路径作为 default tunnel； RN向LN 发起stream connect，反连tcp 流程第一步向 SN call LN， 
                                    //          但是LN没有udp ep，返回错误，进入错误的tunnel dead状态；
                                    //       但是在其他情况， 这个策略可能导致同内网的peer对之间保持一条冗余的tcp 长连接；
                                        || tunnel.as_ref().local().is_tcp() && tunnel.as_ref().remote().addr().port() != 0 && active.default_tunnel.as_ref().remote().addr().port() == 0
                                }
                            };
                            if change_default {
                                info!("{} change default from {} to {}", self, active.default_tunnel.as_ref().as_ref(), tunnel.as_ref().as_ref());
                                let old = Some(active.default_tunnel.clone());
                                active.remote_timestamp = remote_timestamp;
                                active.default_tunnel = tunnel.clone();
                                ((old, Some(tunnel.clone()), None), true)
                            } else {
                                ((None, None, None), false)
                            }
                        }, 
                        TunnelStateImpl::Connecting(connecting) => {
                            info!("{} connecting=>active with default {}", self, tunnel.as_ref().as_ref());
                            let mut ret_waiter = StateWaiter::new();
                            connecting.waiter.transfer_into(&mut ret_waiter);
                            state.tunnel_state = TunnelStateImpl::Active(TunnelActiveState {
                                default_tunnel: tunnel.clone(), 
                                remote_timestamp: remote_timestamp
                            });
                            ((None, Some(tunnel.clone()), Some(ret_waiter)), true)
                        },
                        TunnelStateImpl::Dead(_) => {
                            info!("{} dead=>active with default {}", self, tunnel.as_ref().as_ref());
                            state.tunnel_state = TunnelStateImpl::Active(TunnelActiveState {
                                default_tunnel: tunnel.clone(), 
                                remote_timestamp: remote_timestamp
                            });
                            ((None, Some(tunnel.clone()), None), true)
                        }
                    };
                    if updated {
                        state.last_update = bucky_time_now();
                    }
                    ret
                } else {
                    warn!("{} reset tunnel {} for not in ep map", self, tunnel.as_ref().as_ref());
                    tunnels.push(tunnel.clone());
                    (None, None, None)
                }
            }, 
            TunnelState::Dead => {
                let state = &mut *self.0.state.write().unwrap();
                // 先从entries里面移除
                let entries = &mut state.tunnel_entries;
                let ep_pair = EndpointPair::from((*tunnel.as_ref().local(), *tunnel.as_ref().remote()));
                let exists = {
                    if let Some(stub) = entries.get(&ep_pair) {
                        if stub.as_ref().ptr_eq(tunnel) {
                            info!("{} remove tunnel {}", self, tunnel.as_ref().as_ref());
                            entries.remove(&ep_pair);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };
                
                if exists {
                    if let TunnelState::Active(remote_timestamp) = former_state {
                        match &state.tunnel_state {
                            TunnelStateImpl::Active(active) => {
                                if active.remote_timestamp == remote_timestamp {
                                    let default_tunnel = active.default_tunnel.clone();
                                    info!("{} active=>dead for tunnel {} dead", self, tunnel.as_ref().as_ref());
                                    for (_, tunnel) in &state.tunnel_entries {
                                        tunnels.push(tunnel.clone());
                                    }
                                    state.tunnel_entries.clear();
                                    state.last_update = bucky_time_now();
                                    state.tunnel_state = TunnelStateImpl::Dead(TunnelDeadState {
                                        former_state: TunnelState::Active(active.remote_timestamp), 
                                        when: bucky_time_now()
                                    });
                                    (Some(default_tunnel), None, None)
                                } else {
                                    (None, None, None)
                                }
                            }, 
                            _ => {
                                // do nothing
                                (None, None, None)
                            }
                        }
                    } else {
                        (None, None, None)
                    }
                } else {
                    (None, None, None)
                }
            }
        };
        if let Some(waiter) = waiter {
            waiter.wake();
        }
        if let Some(old) = old {
            old.as_ref().release_keeper();
        }
        if let Some(new) = new {
            new.as_ref().retain_keeper();
        }

        for tunnel in tunnels {
            tunnel.as_ref().reset();
        }
    }

    fn clone_as_tunnel_owner(&self) -> Box<dyn TunnelOwner> {
        Box::new(self.clone())
    }
}

impl OnUdpPackageBox for TunnelContainer {
    fn on_udp_package_box(&self, udp_box: interface::udp::UdpPackageBox) -> Result<(), BuckyError> {
        // 先创建 udp tunnel
        let ep_pair = EndpointPair::from((udp_box.local().local(), *udp_box.remote()));
        let udp_tunnel = match self.tunnel_of::<udp::Tunnel>(&ep_pair) {
            Some(tunnel) => {
                Ok(tunnel)
            }, 
            None => self.create_tunnel::<udp::Tunnel>(ep_pair, ProxyType::None)
        }?;
        // 为了udp 和 tcp tunnel的package 流向一致，直接把box转给udp tunnel，
        // 需要一致处理的package从udp/tcp tunnel回调container的 OnPackage
        udp_tunnel.on_udp_package_box(udp_box)
    }
}

impl OnUdpRawData<(interface::udp::Interface, DeviceId, AesKey, Endpoint, AesKey)> for TunnelContainer {
    fn on_udp_raw_data(&self, data: &[u8], context: (interface::udp::Interface, DeviceId, AesKey, Endpoint, AesKey)) -> Result<(), BuckyError> {
        // // 先创建 udp tunnel
        let (interface, _, mix_key, remote, enc_key) = context;
        let ep_pair = EndpointPair::from((interface.local(), remote));
        let udp_tunnel = match self.tunnel_of::<udp::Tunnel>(&ep_pair) {
            Some(tunnel) => {
                Ok(tunnel)
            }, 
            None => self.create_tunnel::<udp::Tunnel>(ep_pair, ProxyType::None)
        }?;
        // 为了udp 和 tcp tunnel的package 流向一致，直接把box转给udp tunnel，
        // 需要一致处理的package从udp/tcp tunnel回调container的 OnPackage
        let _ = udp_tunnel.active(&mix_key, false, None, &enc_key);
        self.on_raw_data(data)
    }
}

impl OnTcpInterface for TunnelContainer {
    fn on_tcp_interface(&self, interface: interface::tcp::AcceptInterface, first_box: PackageBox) -> Result<OnPackageResult, BuckyError> {
        // 创建tcp tunnel
        let ep_pair = EndpointPair::from((*interface.local(), Endpoint::default_tcp(interface.local())));
        let tcp_tunnel = match self.tunnel_of::<tcp::Tunnel>(&ep_pair) {
            Some(tunnel) => {
                Ok(tunnel)
            }, 
            None => self.create_tunnel::<tcp::Tunnel>(ep_pair, ProxyType::None)
        }?;
        // 为了udp 和 tcp tunnel的package 流向一致，直接把box转给tcp tunnel，
        // 需要一致处理的package从udp/tcp tunnel回调container的 OnPackage
        tcp_tunnel.on_tcp_interface(interface, first_box)
    }
}

impl PingClientCalledEvent<PackageBox> for TunnelContainer {
    fn on_called(&self, called: &SnCalled, caller_box: PackageBox) -> Result<(), BuckyError> {
        let syn_tunnel: &SynTunnel = caller_box.packages_no_exchange()[0].as_ref();
        let remote_timestamp = syn_tunnel.from_device_desc.body().as_ref().unwrap().update_time();
        let _ = self.on_package(syn_tunnel, None)?;
        if let Some(second_pkg) = caller_box.packages_no_exchange().get(1) {
            match second_pkg.cmd_code() {
                PackageCmdCode::SessionData => {
                    let session_data: &SessionData = second_pkg.as_ref();
                    if !session_data.is_syn() {
                        debug!("{} ignore sn called for sesion data not syn", self);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "session data in sn called should has has syn flag"));
                    }
                    let _ = self.on_package(session_data, None)?;
                    let remote_seq = RemoteSequence::from((self.remote().clone(), session_data.syn_info.as_ref().unwrap().sequence));
                    let stream = Stack::from(&self.0.stack).stream_manager().stream_of_remote_sequence(&remote_seq);
                    if stream.is_none() {
                        debug!("{} ignore accept stream builder for stream of {} no more connecting", self, remote_seq);
                        return Ok(());
                    }
                    let stream = stream.unwrap();
                    let acceptor = stream.as_ref().acceptor();
                    if acceptor.is_none() {
                        debug!("{} ignore accept stream builder for stream of {} no more connecting", self, remote_seq);
                        return Ok(());
                    }
                    let acceptor = acceptor.unwrap();
                    //把builder提升到tunnel container，并且开始build
                    match {
                        let mut state = self.0.state.write().unwrap();
                        match &mut state.tunnel_state {
                            TunnelStateImpl::Connecting(connecting) => {
                                match &mut connecting.build_state {
                                    TunnelBuildState::Idle => {
                                        connecting.build_state = TunnelBuildState::AcceptStream(acceptor.clone()); 
                                        Ok((vec![], None))
                                    },
                                    TunnelBuildState::ConnectStream(builder) => Ok((vec![], Some(Box::new(builder.clone()) as Box<dyn PingClientCalledEvent>))), 
                                    TunnelBuildState::ConnectTunnel(builder) => Ok((vec![], Some(Box::new(builder.clone()) as Box<dyn PingClientCalledEvent>))), 
                                    _ => Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "another builder exists"))
                                        // FIXME: how to ?
                                        // another builder exists
                                        // 1. if existing builder is passive, 
                                        //	means that 2 or more sn called package got from sn server before existing builder finish
                                        //		1.1	2nd sn called package has same sequence with 1st, usualy local peer send sn call package to more than one sn server, 
                                        //			or local peer retry building for same connection serveral times, 
                                        //			in this case, ignore this sn called package totally no problem
                                        //		1.2	2nd sn called package has different sequence with 1st, means 1st sn called package sent by connecting 1st connection, but failed,
                                        //			and then 2nd connection connect called caused another builder building for this connection; 
                                        //			or maybe local process exits before building finish and no history log written, 
                                        //			in this case, should cover builder with leatest sequence builder? 
                                        //			or wait existing builder finish? tcp stream can't got establish for local peer's connection instance doesn't exits, 
                                        //			that may cause a long lived mistake that no tcp tunnel can establish but acturely can; 
                                        //			we have to make sure reply all first ack tcp connection package even connecting connection doesn't exist to avoid this
                                        // 2. if existing builder is active,
                                        //		means that local and remote peer create active builder at just same time for connect connection or send package on tunnel, 
                                        //		in this case, we can ignore 2nd builder caused by getting sn called package, the reason is:
                                        //		2.1 if only udp tunnel exists between peers, both sides' builder will finish, 
                                        //			hole punched because both builders are sending syn tunnel package, and then session data reaches
                                        //		2.2 if tcp tunnel exits between peers, tcp interface can establish from at least one side, 
                                        //			connection can establish without builder, the other side's builder may fail, 
                                        //			but connection will use correct reverse stream connector in next retry
                                }
                            }, 
                            TunnelStateImpl::Active(active) => {
                                if active.remote_timestamp < remote_timestamp {
                                    state.last_update = bucky_time_now();
                                    state.tunnel_state = TunnelStateImpl::Connecting(TunnelConnectingState {
                                        waiter: StateWaiter::new(), 
                                        build_state: TunnelBuildState::AcceptStream(acceptor.clone())
                                    });
                                    let mut tunnel_entries = BTreeMap::new();
                                    std::mem::swap(&mut tunnel_entries, &mut state.tunnel_entries);
                                    Ok((tunnel_entries.into_iter().map(|(_, tunnel)| tunnel).collect(), None))
                                } else {
                                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's active"))
                                }
                            }, 
                            TunnelStateImpl::Dead(_) => {
                                state.last_update = bucky_time_now();
                                state.tunnel_state = TunnelStateImpl::Connecting(TunnelConnectingState {
                                    waiter: StateWaiter::new(), 
                                    build_state: TunnelBuildState::AcceptStream(acceptor.clone())
                                });
                                Ok((vec![], None))
                            }   
                        }
                    } {
                        Err(err) => {
                            info!("{} ignore accept stream builder {} for {}", self, acceptor, err);
                        }, 
                        Ok((tunnels, builder)) => {
                            for tunnel in tunnels {
                                tunnel.as_ref().reset();
                            }
                            
                            if let Some(builder) = builder {
                                let _ = builder.on_called(called, ());
                            } else {
                                self.sync_connecting();
                                // 开始被动端的build
                                let _ = acceptor.on_called(called, caller_box);
                            }
                        }
                    }
                }, 
                _ => {
                    //TODO: 支持在sn call带第一个 tunnel 包
                    unreachable!()
                }
            }
        } else if called.reverse_endpoint_array.len() > 0 {
            info!("{} called for reverse connect tunnel", self);
            // let stack = self.stack();
            // let net_listener = stack.net_manager().listener();
            // for local in net_listener.ip_set() {
                for remote in &called.reverse_endpoint_array {
                    let ep_pair = EndpointPair::from((Endpoint::default_tcp(remote), *remote));
                    let tunnel: BuckyResult<tcp::Tunnel> = self.create_tunnel(ep_pair, ProxyType::None);
                    if let Ok(tunnel) = tunnel {
                        let _ = tunnel.connect();
                    }
                }
            // }
        } else {
            let acceptor = AcceptTunnelBuilder::new(self.0.stack.clone(), self.clone(), syn_tunnel.sequence);
            match {
                let mut state = self.0.state.write().unwrap();
                match &mut state.tunnel_state {
                    TunnelStateImpl::Connecting(connecting) => {
                        match &mut connecting.build_state {
                            TunnelBuildState::Idle => {
                                connecting.build_state = TunnelBuildState::AcceptTunnel(acceptor.clone());
                                Ok((vec![], None))
                            },
                            TunnelBuildState::ConnectStream(builder) => Ok((vec![], Some(Box::new(builder.clone()) as Box<dyn PingClientCalledEvent>))), 
                            TunnelBuildState::ConnectTunnel(builder) => Ok((vec![], Some(Box::new(builder.clone()) as Box<dyn PingClientCalledEvent>))), 
                            _ => Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "another builder exists"))
                        }
                    }, 
                    TunnelStateImpl::Active(active) => {
                        if active.remote_timestamp < remote_timestamp {
                            state.last_update = bucky_time_now();
                            state.tunnel_state = TunnelStateImpl::Connecting(TunnelConnectingState {
                                waiter: StateWaiter::new(), 
                                build_state: TunnelBuildState::AcceptTunnel(acceptor.clone())
                            });
                            let mut tunnel_entries = BTreeMap::new();
                            std::mem::swap(&mut tunnel_entries, &mut state.tunnel_entries);
                            Ok((tunnel_entries.into_iter().map(|(_, tunnel)| tunnel).collect(), None))
                        } else {
                            Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's active"))
                        }
                    }, 
                    TunnelStateImpl::Dead(_) => {
                        state.last_update = bucky_time_now();
                        state.tunnel_state = TunnelStateImpl::Connecting(TunnelConnectingState {
                            waiter: StateWaiter::new(), 
                            build_state: TunnelBuildState::AcceptTunnel(acceptor.clone())
                        });
                        Ok((vec![], None))
                    }   
                }
            } {
                Ok((tunnels, builder)) => {
                    for tunnel in tunnels {
                        tunnel.as_ref().reset();
                    }
                    if let Some(builder) = builder {
                        let _ = builder.on_called(called, ());
                    } else {
                        let active_pn_list = called.active_pn_list.clone();
                        self.sync_connecting();
                        // 开始被动端的build
                        task::spawn(async move {
                            let _ = acceptor.build(caller_box, active_pn_list).await;
                        });
                    }
                }, 
                Err(err) => {
                    debug!("{} ignore accept tunnel builder {} for {}", self, acceptor, err);
                }
            }
        }
        Ok(())
    }
}

impl OnPackage<SynTunnel> for TunnelContainer {
    fn on_package(&self, pkg: &SynTunnel, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        // 缓存syn tunnel里面的 desc
        Stack::from(&self.0.stack).device_cache().add(&pkg.from_device_desc.desc().device_id(), &pkg.from_device_desc);
        Ok(OnPackageResult::Handled)
    }
}

impl OnPackage<AckTunnel> for TunnelContainer {
    fn on_package(&self, pkg: &AckTunnel, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        // 缓存ack tunnel里面的 desc
        Stack::from(&self.0.stack).device_cache().add(self.remote(), &pkg.to_device_desc);
        Ok(OnPackageResult::Handled)
    }
}

impl OnPackage<TcpSynConnection, interface::tcp::AcceptInterface> for TunnelContainer {
    fn on_package(&self, pkg: &TcpSynConnection, interface: interface::tcp::AcceptInterface) -> Result<OnPackageResult, BuckyError> {
        // 缓存ack tunnel里面的 desc
        let tunnel_impl = &self.0;
        Stack::from(&tunnel_impl.stack).device_cache().add(&self.remote(), &pkg.from_device_desc);
        // 丢给 stream manager
        Stack::from(&tunnel_impl.stack).stream_manager().on_package(pkg, (self, interface))
            .map_err(|err| {
                debug!("{} handle package {} error {}", self, pkg, err);
                err
            })
    }
}

impl OnPackage<TcpAckConnection, interface::tcp::AcceptInterface> for TunnelContainer {
    fn on_package(&self, pkg: &TcpAckConnection, interface: interface::tcp::AcceptInterface) -> Result<OnPackageResult, BuckyError> {
        // 缓存ack tunnel里面的 desc
        let tunnel_impl = &self.0;
        Stack::from(&tunnel_impl.stack).device_cache().add(&self.remote(), &pkg.to_device_desc);
        // 丢给 stream manager
        Stack::from(&tunnel_impl.stack).stream_manager().on_package(pkg, (self, interface))
            .map_err(|err| {
                debug!("{} handle package {} error {}", self, pkg, err);
                err
            })
    }
}

impl OnPackage<Datagram> for TunnelContainer {
    fn on_package(&self, pkg: &Datagram, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        Stack::from(&self.0.stack).datagram_manager().on_package(pkg, (self, false))
            .map_err(|err| {
                debug!("{} handle package {} error {}", self, pkg, err);
                err
            })
    }
}

impl OnPackage<SessionData> for TunnelContainer {
    fn on_package(&self, pkg: &SessionData, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        let tunnel_impl = &self.0;
        //丢给 stream manager
        Stack::from(&tunnel_impl.stack).stream_manager().on_package(pkg, self)
            .map_err(|err| {
                debug!("{} handle package {} error {}", self, pkg, err);
                err
            })
    }
}

impl OnPackage<TcpSynConnection> for TunnelContainer {
    fn on_package(&self, pkg: &TcpSynConnection, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        let tunnel_impl = &self.0;
        //丢给 stream manager
        Stack::from(&tunnel_impl.stack).stream_manager().on_package(pkg, self)
            .map_err(|err| {
                debug!("{} handle package {} error {}", self, pkg, err);
                err
            })
    }
}

impl OnPackage<AckProxy, &DeviceId> for TunnelContainer {
    fn on_package(&self, pkg: &AckProxy, proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> {
        let tunnel_impl = &self.0;
        let builder = if let TunnelStateImpl::Connecting(connecting) = &tunnel_impl.state.read().unwrap().tunnel_state {
            match &connecting.build_state {
                TunnelBuildState::ConnectStream(builder) => Some(Box::new(builder.clone()) as Box<dyn OnPackage<AckProxy, &DeviceId>>), 
                TunnelBuildState::ConnectTunnel(builder) => Some(Box::new(builder.clone()) as Box<dyn OnPackage<AckProxy, &DeviceId>>), 
                TunnelBuildState::AcceptStream(builder) => Some(Box::new(builder.clone()) as Box<dyn OnPackage<AckProxy, &DeviceId>>), 
                TunnelBuildState::AcceptTunnel(builder) => Some(Box::new(builder.clone()) as Box<dyn OnPackage<AckProxy, &DeviceId>>), 
                TunnelBuildState::Idle => None
            }.ok_or_else(|| BuckyError::new(BuckyErrorCode::ErrorState, "no builder"))
        } else {
            Err(BuckyError::new(BuckyErrorCode::ErrorState, "not connecting"))
        }.map_err(|err| {
            debug!("{} ignore ack proxy from {} for {}", self, proxy, err);
            err
        })?;
        builder.on_package(pkg, proxy)
    }
}



#[derive(Clone)]
pub struct TunnelGuard(Arc<TunnelContainer>);

impl TunnelGuard {
    pub(super) fn new(tunnel: TunnelContainer) -> Self {
        Self(Arc::new(tunnel))
    }

    pub(super) fn mark_in_use(&self) {
        self.0.mark_in_use()
    }

    pub(super) fn mark_recycle(&self, when: Timestamp) -> Option<Timestamp> {
        if Arc::strong_count(&self.0) > 1 {
            None
        } else {
            Some(self.0.mark_recycle(when))
        }
    }
}

impl Deref for TunnelGuard {
    type Target = TunnelContainer;
    fn deref(&self) -> &TunnelContainer {
        &(*self.0)
    }
}
