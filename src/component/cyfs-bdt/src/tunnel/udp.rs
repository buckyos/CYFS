use log::*;
use std::{
    time::Duration, 
    sync::RwLock, 
    sync::atomic::{AtomicI32, AtomicU64, Ordering}
};
use async_std::{
    sync::{Arc}, 
    future
, task};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*,
    protocol::{self, *, v0::*},
    MTU,
    interface::{*, udp::{PackageBoxEncodeContext, OnUdpPackageBox}}, 
};
use super::{
    tunnel::{self, DynamicTunnel, TunnelOwner, ProxyType}, 
    TunnelContainer
};

struct ConnectingState {
    container: TunnelContainer, 
    owner: Box<dyn TunnelOwner>, 
    interface: udp::Interface, 
    waiter: StateWaiter
}

struct ActiveState {
    key: AesKey, 
    // 记录active 这个tunnel时的，远端的 device body 的update time
    remote_timestamp: Timestamp, 
    container: TunnelContainer, 
    owner: Box<dyn TunnelOwner>, 
    interface: udp::Interface, 
}

enum TunnelState {
    Connecting(ConnectingState), 
    Active(ActiveState), 
    Dead
}

impl std::fmt::Display for TunnelState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TunnelState::Connecting(_) => write!(f, "connecting"), 
            TunnelState::Active(active_state) => write!(f, "Active:{{key:{}}}", active_state.key.mix_hash(None).to_string()), 
            TunnelState::Dead => write!(f, "dead")
        }
    }
}

impl From<&TunnelState> for tunnel::TunnelState {
    fn from(state: &TunnelState) -> Self {
        match state {
            TunnelState::Connecting(_) => tunnel::TunnelState::Connecting, 
            TunnelState::Active(active_state) => tunnel::TunnelState::Active(active_state.remote_timestamp), 
            TunnelState::Dead => tunnel::TunnelState::Dead
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub holepunch_interval: Duration, 
    pub connect_timeout: Duration, 
    pub ping_interval: Duration, 
    pub ping_timeout: Duration
}

struct TunnelImpl {
    local: Endpoint, 
    remote: Endpoint,  
    proxy: ProxyType, 
    state: RwLock<TunnelState>, 
    keeper_count: AtomicI32, 
    last_active: AtomicU64,
    mtu: usize,
}

#[derive(Clone)]
pub struct Tunnel(Arc<TunnelImpl>);

impl std::fmt::Display for Tunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UdpTunnel{{local:{},remote:{}}}", tunnel::Tunnel::local(self), tunnel::Tunnel::remote(self))
    }
}

impl Tunnel {
    pub fn new(
        container: TunnelContainer, 
        owner: Box<dyn TunnelOwner>, 
        interface: udp::Interface, 
        remote: Endpoint, 
        proxy: ProxyType) -> Self {
        let local = interface.local();
        let state = TunnelState::Connecting(ConnectingState {
            container: container.clone(), 
            owner: owner.clone_as_tunnel_owner(), 
            interface, 
            waiter: StateWaiter::new()
        });
        let tunnel = Self(Arc::new(TunnelImpl {
            mtu: MTU,
            local, 
            remote, 
            proxy, 
            state: RwLock::new(state), 
            keeper_count: AtomicI32::new(0), 
            last_active: AtomicU64::new(0)
        }));
        
        {
            let tunnel = tunnel.clone();
            let connect_timeout = container.config().udp.connect_timeout;
            task::spawn(async move {
                match future::timeout(connect_timeout, tunnel.wait_active()).await {
                    Ok(_state) => {
                        // assert_eq!(state, tunnel::TunnelState::Active, "state should be active");
                    }, 
                    Err(_err) => {
                        let waiter = {
                            let state = &mut *tunnel.0.state.write().unwrap();
                            match state {
                                TunnelState::Connecting(connecting) => {
                                    let mut waiter = StateWaiter::new();
                                    connecting.waiter.transfer_into(&mut waiter);
                                    *state = TunnelState::Dead;
                                    Some(waiter)
                                }, 
                                TunnelState::Active(_) => {
                                    // do nothing
                                    None
                                },
                                TunnelState::Dead => {
                                    // do nothing
                                    None
                                }
                            }
                        };
                        if let Some(waiter) = waiter  {
                            info!("{} dead for connecting timeout", tunnel);
                            waiter.wake();
                            owner.sync_tunnel_state(&DynamicTunnel::new(tunnel.clone()), tunnel::TunnelState::Connecting, tunnel::TunnelState::Dead);
                        }
                    }
                }
            });
        }

        tunnel
    }

    pub fn try_update_key(&self, by_box: &PackageBox) -> Result<(), BuckyError> {
        let state = &mut *self.0.state.write().unwrap();
        match state {
            TunnelState::Active(active_state) => {
                if active_state.key != *by_box.key() {
                    debug!("{} update active state key from {} to {}", self, active_state.key.mix_hash(None).to_string(), by_box.key().mix_hash(None).to_string());
                    active_state.key = by_box.key().clone();
                    Ok(())
                } else {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "same key"))
                }
            },
            _ => {
                Err(BuckyError::new(BuckyErrorCode::ErrorState, "not active"))
            }
        }
    }

    async fn wait_active(&self) -> tunnel::TunnelState {
        let (state, opt_waiter) = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                TunnelState::Connecting(ref mut connecting_state) => {
                    let waiter = connecting_state.waiter.new_waiter();
                    (tunnel::TunnelState::Connecting, Some(waiter))
                },
                TunnelState::Active(active_state) => {
                    (tunnel::TunnelState::Active(active_state.remote_timestamp), None)
                },
                TunnelState::Dead => {
                    (tunnel::TunnelState::Dead, None)
                }
            }
        };
        if let Some(waiter) = opt_waiter {
            StateWaiter::wait(waiter, | | tunnel::Tunnel::state(self)).await
        } else {
            state
        }
    }

    fn active_by_package(&self, by_box: &PackageBox, remote_timestamp: Option<Timestamp>) -> BuckyResult<TunnelContainer> {
        self.active(by_box.key(), by_box.has_exchange(), remote_timestamp)
    }

    pub fn active(&self, key: &AesKey, exchange: bool, remote_timestamp: Option<Timestamp>) -> BuckyResult<TunnelContainer> { 
        let (container, to_sync, waiter) = {
            let state = &mut *self.0.state.write().unwrap(); 
            match state {
                TunnelState::Connecting(connecting_state) => {
                    if let Some(remote_timestamp) = remote_timestamp {
                        let mut waiter = StateWaiter::new();
                        connecting_state.waiter.transfer_into(&mut waiter);
                        info!("{} change state from Connecting to Active with key:{}", self, key.mix_hash(None).to_string());
                        let owner = connecting_state.owner.clone_as_tunnel_owner();
                        let container = connecting_state.container.clone();
                        *state = TunnelState::Active(ActiveState {
                            container: container.clone(), 
                            owner: owner.clone_as_tunnel_owner(), 
                            remote_timestamp, 
                            interface: connecting_state.interface.clone(),
                            key: key.clone()
                        });
                        Ok((container, 
                            Some((tunnel::TunnelState::Connecting, 
                                tunnel::TunnelState::Active(remote_timestamp), 
                                owner)),  
                            Some(waiter)))
                    } else {
                        Ok((connecting_state.container.clone(), None, None))
                    }
                }, 
                TunnelState::Active(active_state) => {
                    let former_state = tunnel::TunnelState::Active(active_state.remote_timestamp);
                    if let Some(remote_timestamp) = remote_timestamp {
                        if active_state.remote_timestamp < remote_timestamp {
                            debug!("{} update active remote timestamp {}", self, remote_timestamp);
                            active_state.remote_timestamp = remote_timestamp;
                        } 
                    }
                    if exchange && *key != active_state.key {
                        debug!("{} update active state key from {} to {}", self, active_state.key.mix_hash(None).to_string(), key.mix_hash(None).to_string());
                        active_state.key = key.clone();
                    }
                    Ok((active_state.container.clone(), 
                        Some((former_state, 
                            tunnel::TunnelState::Active(active_state.remote_timestamp), 
                            active_state.owner.clone_as_tunnel_owner())), 
                        None))
                },
                TunnelState::Dead => Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's dead"))
            }
        }?;
        
        if let Some(waiter) = waiter {
            waiter.wake();
        }

        if let Some((former_state, new_state, owner)) = to_sync {
            self.0.last_active.store(bucky_time_now(), Ordering::SeqCst);
            if former_state != new_state {
                owner.sync_tunnel_state(&tunnel::DynamicTunnel::new(self.clone()), former_state, new_state);
            }
        }

        Ok(container)
    }

    pub fn send_box(&self, package_box: &PackageBox) -> Result<(), BuckyError> {
        let (interface, tunnel_container) = {
            let state = &*self.0.state.read().unwrap();
            match state {
                TunnelState::Connecting(connecting) => Ok((connecting.interface.clone(), connecting.container.clone())), 
                TunnelState::Active(active) => Ok((active.interface.clone(), active.container.clone())), 
                TunnelState::Dead => Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel dead"))
            }
        }?;
        let mut context = PackageBoxEncodeContext::from(tunnel_container.remote_const());
        context.set_ignore_exchange(ProxyType::None != self.0.proxy);
        interface.send_box_to(&mut context, package_box, tunnel::Tunnel::remote(self))?;
        Ok(())
    }

    pub fn raw_data_max_len() -> usize {
        udp::MTU
    }

    pub(super) fn raw_data_header_len_impl() -> usize {
        KeyMixHash::raw_bytes().unwrap()
    }

    pub fn raw_data_max_payload_len() -> usize {
        Self::raw_data_max_len() - Self::raw_data_header_len_impl()
    }


    fn owner(&self) -> Option<TunnelContainer> {
        let state = &*self.0.state.read().unwrap();
        match state {
            TunnelState::Connecting(connecting) => Some(connecting.container.clone()),  
            TunnelState::Active(active) => Some(active.container.clone()), 
            TunnelState::Dead => None
        }
    }
}

#[async_trait]
impl tunnel::Tunnel for Tunnel {
    fn mtu(&self) -> usize {
        self.0.mtu
    }

    fn ptr_eq(&self, other: &tunnel::DynamicTunnel) -> bool {
        *self.local() == *other.as_ref().local() 
        && *self.remote() == *other.as_ref().remote()
        && Arc::ptr_eq(&self.0, &other.clone_as_tunnel::<Tunnel>().0)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn local(&self) -> &Endpoint {
        &self.0.local
    }

    fn remote(&self) -> &Endpoint {
        &self.0.remote
    }

    fn proxy(&self) -> ProxyType {
        self.0.proxy.clone()
    }

    fn state(&self) -> tunnel::TunnelState {
        let state = &*self.0.state.read().unwrap();
        tunnel::TunnelState::from(state)
    }

    
    fn raw_data_header_len(&self) -> usize {
        Self::raw_data_header_len_impl()
    }

    fn send_raw_data(&self, data: &mut [u8]) -> Result<usize, BuckyError> {
        let (key, interface) = {
            let state = &*self.0.state.read().unwrap();
            match state {
                TunnelState::Connecting(_) => Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel not active")), 
                TunnelState::Active(active) => Ok((active.key.clone(), active.interface.clone())), 
                TunnelState::Dead => Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel dead"))
            }
        }?;

        assert_eq!(data.len() > Self::raw_data_header_len_impl(), true);
        
        interface.send_raw_data_to(&key, data, tunnel::Tunnel::remote(self))
    }

    fn send_package(&self, package: DynamicPackage) -> Result<(), BuckyError> {
        let (tunnel_container, interface, key) = {
            if let TunnelState::Active(active_state) =  &*self.0.state.read().unwrap() {
            Ok((active_state.container.clone(), active_state.interface.clone(), active_state.key.clone()))
        } else {
            Err(BuckyError::new(BuckyErrorCode::ErrorState, "send packages on tunnel not active"))
        }}?;
        trace!("{} send packages with key {}", self, key.mix_hash(None).to_string());
        let package_box = PackageBox::from_package(tunnel_container.remote().clone(), key, package);
        let mut context = PackageBoxEncodeContext::from(tunnel_container.remote_const());
        context.set_ignore_exchange(ProxyType::None != self.0.proxy);
        interface.send_box_to(&mut context, &package_box, tunnel::Tunnel::remote(self))?;
        Ok(())
    }

    fn retain_keeper(&self) {
        info!("{} retain keeper", self);
        if 0 == self.0.keeper_count.fetch_add(1, Ordering::SeqCst) {
            if let Some((container, owner, cur_state)) = {
                let state = &*self.0.state.write().unwrap();
                if let TunnelState::Active(active_state) = state {
                    Some((active_state.container.clone(),
                    active_state.owner.clone_as_tunnel_owner(),  
                    tunnel::TunnelState::Active(active_state.remote_timestamp)))
                } else {
                    None
                }
            } {
                let tunnel = self.clone();
                let ping_interval = container.config().udp.ping_interval;
                let ping_timeout = container.config().udp.ping_timeout;

                task::spawn(async move {
                    loop {
                        if tunnel.0.keeper_count.load(Ordering::SeqCst) == 0 {
                            break;
                        }
                        let now = bucky_time_now();
                        let miss_active_time = Duration::from_micros(now - tunnel.0.last_active.load(Ordering::SeqCst));
                        if miss_active_time > ping_timeout {
                            let state = &mut *tunnel.0.state.write().unwrap();
                            if let TunnelState::Active(_) = state {
                                info!("{} dead for ping timeout", tunnel);
                                *state = TunnelState::Dead;
                                break;
                            } else {
                                break;
                            }
                        }
                        if miss_active_time > ping_interval {
                            if tunnel.0.keeper_count.load(Ordering::SeqCst) > 0 {
                                debug!("{} send ping", tunnel);
                                let ping = PingTunnel {
                                    package_id: 0,
                                    send_time: now,
                                    recv_data: 0,
                                };
                                let _ = tunnel::Tunnel::send_package(&tunnel, DynamicPackage::from(ping));
                            }
                        }

                        let _ = future::timeout(ping_interval, future::pending::<()>()).await;
                    };
                    owner.sync_tunnel_state(&tunnel::DynamicTunnel::new(tunnel.clone()), cur_state, tunnel.state());
                });
            } else {
                return;
            }
        }
    }

    fn release_keeper(&self) {
        info!("{} release keeper", self);
        self.0.keeper_count.fetch_add(-1, Ordering::SeqCst);
    }

    fn reset(&self) {
        info!("{} reset to Dead", self);
        let mut state = self.0.state.write().unwrap();
        *state = TunnelState::Dead;
    }
}

impl OnUdpPackageBox for Tunnel {
    fn on_udp_package_box(&self, udp_box: udp::UdpPackageBox) -> Result<(), BuckyError> {
        for p in udp_box.as_ref().packages_no_exchange() {
            match downcast_tunnel_handle!(p, |p| self.on_package(p, udp_box.as_ref()))? {
                OnPackageResult::Break => break, 
                _ => continue,
            };
        };
        Ok(())
        
    }
}


impl OnPackage<SynTunnel, &PackageBox> for Tunnel {
    fn on_package(&self, syn_tunnel: &SynTunnel, in_box: &PackageBox) -> Result<OnPackageResult, BuckyError> {
        let container = self.active_by_package(in_box, Some(syn_tunnel.from_device_desc.body().as_ref().unwrap().update_time()))?;
        // TODO: 考虑合并ack 和 session data
        // 回复ack tunnel
        let ack = AckTunnel {
            protocol_version: container.protocol_version(), 
            stack_version: container.stack_version(), 
            sequence: syn_tunnel.sequence,
            result: 0,
            send_time: 0,
            mtu: udp::MTU as u16,
            to_device_desc: container.stack().device_cache().local()       
        };

        let mut package_box = PackageBox::encrypt_box(container.remote().clone(), in_box.key().clone());
        package_box.append(vec![DynamicPackage::from(ack)]);
        let _ = self.send_box(&package_box);
         // 传回给 container 处理
         container.on_package(syn_tunnel, None)
    }
}

impl OnPackage<AckTunnel, &PackageBox> for Tunnel {
    fn on_package(&self, pkg: &AckTunnel, in_box: &PackageBox) -> Result<OnPackageResult, BuckyError> {
        let container = self.active_by_package(in_box, Some(pkg.to_device_desc.body().as_ref().unwrap().update_time()))?;
        // 传回给 container 处理
        container.on_package(pkg, None)
    }
}

impl OnPackage<AckAckTunnel, &PackageBox> for Tunnel {
    fn on_package(&self, _: &AckAckTunnel, in_box: &PackageBox) -> Result<OnPackageResult, BuckyError> {
        let _ = self.active_by_package(in_box, None)?;
        // do nothing
        Ok(OnPackageResult::Handled)
    }
}

impl OnPackage<PingTunnel, &PackageBox> for Tunnel {
    fn on_package(&self, ping: &PingTunnel, in_box: &PackageBox) -> Result<OnPackageResult, BuckyError> {
        let _ = self.active_by_package(in_box, None)?;
        let ping_resp = PingTunnelResp {
            ack_package_id: ping.package_id,
            send_time: bucky_time_now(),
            recv_data: 0,
        };
        let _ = tunnel::Tunnel::send_package(self, DynamicPackage::from(ping_resp));
        Ok(OnPackageResult::Handled)
    }
}

impl OnPackage<PingTunnelResp, &PackageBox> for Tunnel {
    fn on_package(&self, _: &PingTunnelResp, in_box: &PackageBox) -> Result<OnPackageResult, BuckyError> {
        let _ = self.active_by_package(in_box, None)?;
        // do nothing
        Ok(OnPackageResult::Handled)
    }
}

impl OnPackage<Datagram, &PackageBox> for Tunnel {
    fn on_package(&self, pkg: &Datagram, in_box: &PackageBox) -> Result<OnPackageResult, BuckyError> {
        let container = self.active_by_package(in_box, None)?;
        // 传回给 container 处理
        container.on_package(pkg, None)
    }
}

impl OnPackage<SessionData, &PackageBox> for Tunnel {
    fn on_package(&self, pkg: &SessionData, in_box: &PackageBox) -> Result<OnPackageResult, BuckyError> {
        let container = self.active_by_package(in_box, None)?;
        // 传回给 container 处理
        container.on_package(pkg, None)
    }
}

impl OnPackage<TcpSynConnection, &PackageBox> for Tunnel {
    fn on_package(&self, pkg: &TcpSynConnection, in_box: &PackageBox) -> Result<OnPackageResult, BuckyError> {
        let container = self.active_by_package(in_box, None)?;
        // 传回给 container 处理
        container.on_package(pkg, None)
    }
}










