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
    interface::*, 
    history::keystore, 
    sn::client::PingClientCalledEvent, 
    tunnel::{TunnelState, TunnelContainer, ProxyType, BuildTunnelParams}, 
    stack::{Stack, WeakStack}
};
use super::super::{
    action::*, 
    builder::*, 
    proxy::*
};

struct ConnectingState {
    proxy: Option<ProxyBuilder>, 
    waiter: StateWaiter
}

enum ConnectTunnelBuilderState {
    Connecting(ConnectingState), 
    Establish, 
    Closed
}

struct ConnectTunnelBuilderImpl {
    stack: WeakStack, 
    tunnel: TunnelContainer,
    params: BuildTunnelParams, 
    sequence: TempSeq,
    state: RwLock<ConnectTunnelBuilderState>
}

#[derive(Clone)]
pub struct ConnectTunnelBuilder(Arc<ConnectTunnelBuilderImpl>);

impl std::fmt::Display for ConnectTunnelBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConnectTunnelBuilder{{tunnel:{}}}", self.0.tunnel)
    }
}

impl ConnectTunnelBuilder {
    pub fn new(stack: WeakStack, tunnel: TunnelContainer, params: BuildTunnelParams) -> Self {
        let sequence = tunnel.generate_sequence();
        Self(Arc::new(ConnectTunnelBuilderImpl {
            stack, 
            tunnel,
            params, 
            sequence, 
            state: RwLock::new(ConnectTunnelBuilderState::Connecting(ConnectingState {
                proxy: None, 
                waiter:StateWaiter::new()
            }))
        }))
    }
    pub async fn build(&self) {
        self.sync_tunnel_state();
        let stack = Stack::from(&self.0.stack);
        let local = stack.local().clone();
        let build_params = &self.0.params;

        let first_box = Arc::new(self.first_box(&local).await);

        let actions = if let Some(remote) = build_params.remote_desc.as_ref() {
            self.explore_endpoint_pair(remote, first_box.clone(), |ep| ep.is_static_wan())
        } else {
            vec![]
        };
   
        if actions.len() == 0 {
            match {
                if let Some(sn) = if build_params.remote_sn.len() == 0 {
                    stack.device_cache().get_nearest_of(&build_params.remote_const.device_id())
                } else {
                    stack.device_cache().get(&build_params.remote_sn[0]).await
                } {
                    match self.call_sn(sn, first_box).await {
                        Ok(actions) => {
                            if actions.len() == 0 {
                                Err(BuckyError::new(BuckyErrorCode::NotConnected, "on endpoint pair can establish"))
                            } else {
                                Ok(actions)
                            }
                        },
                        Err(err) => {
                            let msg = format!("call sn err:{}", err.msg());
                            Err(BuckyError::new(err.code(), msg.as_str()))
                        }
                    }
                } else {
                    Err(BuckyError::new(BuckyErrorCode::InvalidParam, "got sn device object failed"))
                }
            } {
                Ok(_actions) => {
                    // do nothing
                }, 
                Err(err) => {
                    error!("{} build failed for {}", self, err);
                    let waiter = {
                        let state = &mut *self.0.state.write().unwrap();
                        match state {
                            ConnectTunnelBuilderState::Connecting(connecting) => {
                                info!("{} connecting=>dead", self);
                                let mut ret_waiter = StateWaiter::new();
                                connecting.waiter.transfer_into(&mut ret_waiter);
                                *state = ConnectTunnelBuilderState::Closed;
                                Some(ret_waiter)
                            }, 
                            ConnectTunnelBuilderState::Closed => {
                                //存在closed之后tunnel dead的情况，忽略
                                None
                            }, 
                            ConnectTunnelBuilderState::Establish => {
                                //存在establish之后tunnel dead的情况，忽略
                                None
                            }
                        }
                    };
                    if let Some(waiter) = waiter {
                        waiter.wake();
                    }
                }
            }
        }
    }

    fn sync_tunnel_state(&self) {
        let builder = self.clone();
        task::spawn(async move {
            let tunnel_state = builder.0.tunnel.wait_active().await;
            let waiter = match tunnel_state {
                TunnelState::Active(_) => {
                    let state = &mut *builder.0.state.write().unwrap();
                    match state {
                        ConnectTunnelBuilderState::Connecting(connecting) => {
                            info!("{} connecting=>establish", builder);
                            let mut ret_waiter = StateWaiter::new();
                            connecting.waiter.transfer_into(&mut ret_waiter);
                            *state = ConnectTunnelBuilderState::Establish;
                            Some(ret_waiter)
                        }, 
                        ConnectTunnelBuilderState::Closed => {
                            //存在closed之后tunnel联通的情况，忽略
                            None
                        }, 
                        ConnectTunnelBuilderState::Establish => {
                            unreachable!()
                        }
                    }
                }, 
                TunnelState::Dead | TunnelState::Connecting => {
                    let state = &mut *builder.0.state.write().unwrap();
                    match state {
                        ConnectTunnelBuilderState::Connecting(connecting) => {
                            info!("{} connecting=>dead", builder);
                            let mut ret_waiter = StateWaiter::new();
                            connecting.waiter.transfer_into(&mut ret_waiter);
                            *state = ConnectTunnelBuilderState::Closed;
                            Some(ret_waiter)
                        }, 
                        ConnectTunnelBuilderState::Closed => {
                            //存在closed之后tunnel dead的情况，忽略
                            None
                        }, 
                        ConnectTunnelBuilderState::Establish => {
                            //存在establish之后tunnel dead的情况，忽略
                            None
                        }
                    }
                },
            };
            if let Some(waiter) = waiter {
                waiter.wake();
            }
        });
    }

    async fn call_sn(&self, sn: Device, first_box: Arc<PackageBox>) -> Result<Vec<DynBuildTunnelAction>, BuckyError> {
        let stack = Stack::from(&self.0.stack);
        let tunnel = &self.0.tunnel;

        let remote = stack.sn_client().call(
            &vec![],  
            tunnel.remote(),
            &sn, 
            true, 
            true,
            false,
            |sn_call| {
                let mut context = udp::PackageBoxEncodeContext::from(sn_call);
                //FIXME 先不调用raw_measure_with_context
                //let len = first_box.raw_measure_with_context(&mut context).unwrap();
                let mut buf = vec![0u8; 2048];
                let b = first_box.raw_encode_with_context(&mut buf, &mut context, &None).unwrap();
                //buf[0..b.len()].as_ref()
                let len = 2048 - b.len();
                buf.truncate(len);
                buf
            }).await?;

        if let Some(proxy_buidler) = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                ConnectTunnelBuilderState::Connecting(connecting) => {
                    if connecting.proxy.is_none() {
                        let proxy = ProxyBuilder::new(
                            tunnel.clone(), 
                            remote.get_obj_update_time(),  
                            first_box.clone());
                        debug!("{} create proxy builder", self);
                        connecting.proxy = Some(proxy);
                    }
                    connecting.proxy.clone()
                }, 
                _ => {
                    debug!("{} ignore proxy builder for not in connecting1 state", self);
                    None
                }
            }
        } {
            //FIXME: 使用正确的proxy策略
            for proxy in stack.proxy_manager().active_proxies() {
                let _ = proxy_buidler.syn_proxy(ProxyType::Active(proxy)).await;
            }
            for proxy in remote.connect_info().passive_pn_list().iter().cloned() {
                let _ = proxy_buidler.syn_proxy(ProxyType::Passive(proxy)).await;
            }
        }

        Ok(self.explore_endpoint_pair(&remote, first_box, |_| true))
    }

    fn explore_endpoint_pair<F: Fn(&Endpoint) -> bool>(&self, remote: &Device, first_box: Arc<PackageBox>, filter: F) -> Vec<DynBuildTunnelAction> {
        let stack = Stack::from(&self.0.stack);
        let tunnel = &self.0.tunnel;
        let net_listener = stack.net_manager().listener();

        let mut actions = vec![];

        let connect_info = remote.connect_info();
        for udp_interface in net_listener.udp() {
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_udp() && ep.is_same_ip_version(&udp_interface.local()) && filter(ep)) {
                if let Ok(udp_tunnel) = tunnel.create_tunnel(EndpointPair::from((udp_interface.local(), *remote_ep)), ProxyType::None) {
                    let action = SynUdpTunnel::new(
                        udp_tunnel, 
                        first_box.clone(), 
                        tunnel.config().udp.holepunch_interval); 
                    actions.push(Box::new(action) as DynBuildTunnelAction);
                }  
            }    
        }

        // for local_ip in net_listener.ip_set() {
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_tcp() && filter(ep)) {
                if let Ok(tunnel) = tunnel.create_tunnel(EndpointPair::from((Endpoint::default_tcp(remote_ep), *remote_ep)), ProxyType::None) {
                    let action = ConnectTcpTunnel::new(tunnel);
                    actions.push(Box::new(action) as DynBuildTunnelAction);
                }   
            }   
        // }

        actions
    }  

    //FXIME: 这里有机会把要发的一个session包放进来
    async fn first_box(&self, local: &Device) -> PackageBox {
        let stack = Stack::from(&self.0.stack);
        let tunnel = &self.0.tunnel;

        let key_stub = stack.keystore().create_key(tunnel.remote_const(), true);
        // 生成第一个package box
        let mut first_box = PackageBox::encrypt_box(tunnel.remote().clone(), key_stub.key.clone());
            
        let syn_tunnel = SynTunnel {
            protocol_version: self.0.tunnel.protocol_version(), 
            stack_version: self.0.tunnel.stack_version(), 
            to_device_id: tunnel.remote().clone(), 
            from_device_desc: local.clone(),
            sequence: self.sequence(), 
            send_time: bucky_time_now()
        };
        if let keystore::EncryptedKey::Unconfirmed(key_encrypted) = key_stub.encrypted {
            let mut exchange = Exchange::from((&syn_tunnel, key_encrypted, key_stub.key.mix_key));
            let _ = exchange.sign(stack.keystore().signer()).await;
            first_box.push(exchange);
        }
        first_box.push(syn_tunnel);
        first_box
    }
}

#[async_trait]
impl TunnelBuilder for ConnectTunnelBuilder {
    fn sequence(&self) -> TempSeq {
        self.0.sequence
    }
    fn state(&self) -> TunnelBuilderState {
        match &*self.0.state.read().unwrap() {
            ConnectTunnelBuilderState::Connecting(_) => TunnelBuilderState::Connecting, 
            ConnectTunnelBuilderState::Establish => TunnelBuilderState::Establish,
            ConnectTunnelBuilderState::Closed => TunnelBuilderState::Closed
        }
    }
    async fn wait_establish(&self) -> Result<(), BuckyError> {
        let (state, waiter) = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                ConnectTunnelBuilderState::Connecting(connecting) => {
                    (TunnelBuilderState::Connecting, Some(connecting.waiter.new_waiter()))
                },
                ConnectTunnelBuilderState::Establish => {
                    (TunnelBuilderState::Establish, None)
                },
                ConnectTunnelBuilderState::Closed => {
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

impl PingClientCalledEvent for ConnectTunnelBuilder {
    fn on_called(&self, called: &SnCalled, _context: ()) -> Result<(), BuckyError> {
        let builder = self.clone();
        let active_pn_list = called.active_pn_list.clone();
        let remote_timestamp = called.peer_info.get_obj_update_time();
        task::spawn(async move {
            let stack = Stack::from(&builder.0.stack);
            let first_box = builder.first_box(&stack.device_cache().local()).await;
            if let Some(proxy_builder) = {
                let state = &mut *builder.0.state.write().unwrap();
                match state {
                    ConnectTunnelBuilderState::Connecting(connecting) => {
                        if connecting.proxy.is_none() {
                            let proxy = ProxyBuilder::new(
                                builder.0.tunnel.clone(), 
                                remote_timestamp,  
                                Arc::new(first_box));
                            debug!("{} create proxy builder", builder);
                            connecting.proxy = Some(proxy);
                        }
                        connecting.proxy.clone()
                    }, 
                    _ => None
                }
            } {
                //FIXME: 使用正确的proxy策略
                for proxy in active_pn_list {
                    let _ = proxy_builder.syn_proxy(ProxyType::Active(proxy));
                }
            }
        });
        
        Ok(())
    }
}


impl OnPackage<AckProxy, &DeviceId> for ConnectTunnelBuilder {
    fn on_package(&self, pkg: &AckProxy, proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> {
        if let Some(proxy_builder) = match &*self.0.state.read().unwrap() {
            ConnectTunnelBuilderState::Connecting(connecting) => {
                connecting.proxy.clone()
            }, 
            _ => {
                None
            }
        } {
            proxy_builder.on_package(pkg, proxy)
        } else {
            let err = BuckyError::new(BuckyErrorCode::ErrorState, "proxy builder not exists");
            debug!("{} ignore ack proxy from {} for {}", self, proxy, err);
            Err(err)
        }
    }
}




