use log::*;
use std::{
    sync::RwLock
};
use async_std::{sync::{Arc}, task};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*},
    tunnel::{TunnelState, ProxyType, TunnelContainer}, 
    stack::{Stack, WeakStack}
};
use super::{
    action::*, 
    builder::*, 
    proxy::*
};


struct ConnectingState {
    waiter: StateWaiter, 
    proxy: Option<ProxyBuilder>
}

enum AcceptTunnelBuilderState {
    Connecting(ConnectingState), 
    Establish, 
    Closed
}

struct AcceptTunnelBuilderImpl {
    stack: WeakStack, 
    tunnel: TunnelContainer, 
    sequence: TempSeq, 
    state: RwLock<AcceptTunnelBuilderState>
}

#[derive(Clone)]
pub struct AcceptTunnelBuilder(Arc<AcceptTunnelBuilderImpl>);

impl std::fmt::Display for AcceptTunnelBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AcceptTunnelBuilder{{tunnel:{}}}", self.0.tunnel)
    }
}

impl AcceptTunnelBuilder {
    pub fn new(stack: WeakStack, tunnel: TunnelContainer, sequence: TempSeq) -> Self {
        Self(Arc::new(AcceptTunnelBuilderImpl {
            stack, 
            tunnel, 
            sequence, 
            state: RwLock::new(AcceptTunnelBuilderState::Connecting(ConnectingState {
                waiter: StateWaiter::new(), 
                proxy: None
            }))
        }))
    }

    pub async fn build(&self, caller_box: PackageBox, active_pn_list: Vec<DeviceId>) -> Result<(), BuckyError> {
        info!("{} build", self);
        self.sync_tunnel_state();
        {
            let stack = Stack::from(&self.0.stack);
            let local = stack.sn_client().ping().default_local();
            let syn_tunnel: &SynTunnel = caller_box.packages_no_exchange()[0].as_ref();           
            // first box 包含 ack tunnel 和 session data
            let tunnel = &self.0.tunnel;
            let ack_tunnel = SynTunnel {
                protocol_version: tunnel.protocol_version(), 
                stack_version: tunnel.stack_version(), 
                to_device_id: syn_tunnel.from_device_desc.desc().device_id(),
                sequence: syn_tunnel.sequence,
                from_device_desc: local,
                send_time: 0
            };
            let mut first_box = PackageBox::encrypt_box(caller_box.remote().clone(), caller_box.key().clone());
            first_box.append(vec![DynamicPackage::from(ack_tunnel)]);
            let first_box = Arc::new(first_box);
            let _ = self.explore_endpoint_pair(&syn_tunnel.from_device_desc, first_box.clone(), |_| true);

            if let Some(proxy_builder) = {
                let state = &mut *self.0.state.write().unwrap();
                match state {
                    AcceptTunnelBuilderState::Connecting(connecting) => {
                        if connecting.proxy.is_none() {
                            connecting.proxy = Some(ProxyBuilder::new(
                                self.0.tunnel.clone(), 
                                syn_tunnel.from_device_desc.get_obj_update_time(),  
                                first_box.clone()));
                            debug!("{} create proxy buidler", self);
                        }
                        connecting.proxy.clone()
                    },
                    _ => None
                }
            } {
                for proxy in active_pn_list {
                    let _ = proxy_builder.syn_proxy(ProxyType::Active(proxy)).await;
                }
                for proxy in stack.proxy_manager().passive_proxies() {
                    let _ = proxy_builder.syn_proxy(ProxyType::Passive(proxy)).await;
                }
            } else {
                debug!("{} ignore proxy build for not connecting", self);
            }

            Ok(())
        }.map_err(|e| {info!("{} ingnore build for {}", self, e);e})
    }

    fn sync_tunnel_state(&self) {
        let builder = self.clone();
        task::spawn(async move {
            let tunnel_state = builder.0.tunnel.wait_active().await;
            let waiter = match tunnel_state {
                TunnelState::Active(_) => {
                    let state = &mut *builder.0.state.write().unwrap();
                    match state {
                        AcceptTunnelBuilderState::Connecting(connecting) => {
                            info!("{} connecting=>establish", builder);
                            let mut ret_waiter = StateWaiter::new();
                            connecting.waiter.transfer_into(&mut ret_waiter);
                            *state = AcceptTunnelBuilderState::Establish;
                            Some(ret_waiter)
                        }, 
                        AcceptTunnelBuilderState::Closed => {
                            //存在closed之后tunnel联通的情况，忽略
                            None
                        }, 
                        AcceptTunnelBuilderState::Establish => {
                            unreachable!()
                        }
                    }
                }, 
                TunnelState::Dead => {
                    let state = &mut *builder.0.state.write().unwrap();
                    match state {
                        AcceptTunnelBuilderState::Connecting(connecting) => {
                            info!("{} connecting=>dead", builder);
                            let mut ret_waiter = StateWaiter::new();
                            connecting.waiter.transfer_into(&mut ret_waiter);
                            *state = AcceptTunnelBuilderState::Closed;
                            Some(ret_waiter)
                        }, 
                        AcceptTunnelBuilderState::Closed => {
                            //存在closed之后tunnel dead的情况，忽略
                            None
                        }, 
                        AcceptTunnelBuilderState::Establish => {
                            //存在establish之后tunnel dead的情况，忽略
                            None
                        }
                    }
                }, 
                TunnelState::Connecting => {
                    unreachable!()
                }
            };
            if let Some(waiter) = waiter {
                waiter.wake();
            }
        });
    }


    fn explore_endpoint_pair<F: Fn(&Endpoint) -> bool>(&self, remote: &Device, first_box: Arc<PackageBox>, filter: F) -> Vec<DynBuildTunnelAction> {
        let stack = Stack::from(&self.0.stack);
        let tunnel = &self.0.tunnel;
        let net_listener = stack.net_manager().listener();

        let mut actions = vec![];

        let connect_info = remote.connect_info();
        for udp_interface in net_listener.udp() {
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_udp() && ep.is_same_ip_version(&udp_interface.local()) && (ep.addr().is_ipv6() || (ep.addr().is_ipv4() && filter(ep)))) {
                if let Ok((udp_tunnel, newly_created)) = tunnel.create_tunnel(EndpointPair::from((udp_interface.local(), *remote_ep)), ProxyType::None) {
                    if newly_created {
                        let action = SynUdpTunnel::new(
                            udp_tunnel, 
                            first_box.clone(), 
                            tunnel.config().udp.holepunch_interval); 
                        actions.push(Box::new(action) as DynBuildTunnelAction);
                    }
                }      
            }
        }

        // for local_ip in net_listener.ip_set() {
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_tcp() && (ep.addr().is_ipv6() || (ep.addr().is_ipv4() && filter(ep)))) {
                if let Ok((tunnel, newly_created)) = tunnel.create_tunnel(EndpointPair::from((Endpoint::default_tcp(remote_ep), *remote_ep)), ProxyType::None) {
                    if newly_created {
                        let action = ConnectTcpTunnel::new(tunnel);
                        actions.push(Box::new(action) as DynBuildTunnelAction);
                    }
                }    
            }  
        // }

        actions
    }  
}

#[async_trait]
impl TunnelBuilder for AcceptTunnelBuilder {
    fn sequence(&self) -> TempSeq {
        self.0.sequence
    }
    fn state(&self) -> TunnelBuilderState {
        match &*self.0.state.read().unwrap() {
            AcceptTunnelBuilderState::Connecting(_) => TunnelBuilderState::Connecting, 
            AcceptTunnelBuilderState::Establish => TunnelBuilderState::Establish,
            AcceptTunnelBuilderState::Closed => TunnelBuilderState::Closed
        }
    }
    async fn wait_establish(&self) -> Result<(), BuckyError> {
        let (state, waiter) = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                AcceptTunnelBuilderState::Connecting(connecting) => {
                    (TunnelBuilderState::Connecting, Some(connecting.waiter.new_waiter()))
                },
                AcceptTunnelBuilderState::Establish => {
                    (TunnelBuilderState::Establish, None)
                },
                AcceptTunnelBuilderState::Closed => {
                    (TunnelBuilderState::Closed, None)
                }
            }
        };
        match if let Some(waiter) = waiter {
            StateWaiter::wait(waiter, | | self.state()).await
        } else {
            state
        } {
            TunnelBuilderState::Establish => Ok(()), 
            TunnelBuilderState::Closed => Err(BuckyError::new(BuckyErrorCode::Failed, "builder failed")),
            TunnelBuilderState::Connecting => unreachable!()
        }
    }
}

impl OnPackage<AckProxy, &DeviceId> for AcceptTunnelBuilder {
    fn on_package(&self, pkg: &AckProxy, proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> {
        if let Some(proxy_builder) = match &*self.0.state.read().unwrap() {
            AcceptTunnelBuilderState::Connecting(connecting) => connecting.proxy.clone(),
            _ => None
        } {
            proxy_builder.on_package(pkg, proxy)
        } else {
            let err = BuckyError::new(BuckyErrorCode::ErrorState, "proxy builder not exists");
            debug!("{} ignore ack proxy from {} for {}", self, proxy, err);
            Err(err)
        }
    }
}




