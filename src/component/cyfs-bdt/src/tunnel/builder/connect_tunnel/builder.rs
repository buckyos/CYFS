use log::*;
use std::{
    sync::RwLock, 
    time::Duration
};
use async_std::{
    sync::{Arc}, 
    task,
    future
};
use futures::future::{Abortable, AbortHandle};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*},
    interface::{*, udp::MTU_LARGE}, 
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
    start_at: Timestamp, 
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
            start_at: bucky_time_now(), 
            tunnel,
            params, 
            sequence, 
            state: RwLock::new(ConnectTunnelBuilderState::Connecting(ConnectingState {
                proxy: None, 
                waiter:StateWaiter::new()
            }))
        }))
    }

    fn escaped(&self) -> Duration {
        let now = bucky_time_now();
        if now > self.0.start_at {
            Duration::from_micros(now - self.0.start_at)
        } else {
            Duration::from_micros(0)
        }
    }

    async fn build_inner(&self) -> BuckyResult<()> {
        let stack = Stack::from(&self.0.stack);
        let local = stack.sn_client().ping().default_local();
        let build_params = &self.0.params;

        let first_box = Arc::new(self.first_box(&local).await);

        info!("{} build with key {}", self, first_box.key());
        let remote_id = build_params.remote_const.device_id();
        let cached_remote = stack.device_cache().get_inner(&remote_id);
        let known_remote = cached_remote.as_ref().or_else(|| build_params.remote_desc.as_ref());

        let actions = if let Some(remote) = known_remote {
            info!("{} explore_endpoint_pair with known remote {:?}", self, remote.connect_info().endpoints());
            self.explore_endpoint_pair(remote, first_box.clone(), |ep| ep.is_static_wan())
        } else {
            vec![]
        };
   
        if actions.len() == 0 {
            let nearest_sn = build_params.nearest_sn(&stack);
            if let Some(sn) = nearest_sn {
                info!("{} call nearest sn, sn={}", self, sn);
                let timeout_ret = future::timeout(stack.config().stream.stream.retry_sn_timeout, self.call_sn(vec![sn.clone()], first_box.clone())).await;
                let retry_sn_list = match timeout_ret {
                    Ok(finish_ret) => {
                        match finish_ret {
                            Ok(_) => {
                                info!("{} call nearest sn finished, sn={}", self, sn);
                                if TunnelBuilderState::Establish != self.state() {
                                    let escaped = self.escaped();
                                    if stack.config().stream.stream.retry_sn_timeout > escaped {
                                        Some(Duration::from_secs(0))
                                    } else {
                                        Some(stack.config().stream.stream.retry_sn_timeout - escaped)
                                    }
                                } else {
                                    None
                                }
                            }, 
                            Err(err) => {
                                if err.code() == BuckyErrorCode::Interrupted {
                                    info!("{} call nearest sn canceled, sn={}", self, sn);
                                    None
                                } else {
                                    error!("{} call nearest sn failed, sn={}, err={}", self, sn, err);
                                    Some(Duration::from_secs(0))
                                }
                            }
                        }
                    },
                    Err(_) => {
                        warn!("{} call nearest sn timeout {}", self, sn);
                        Some(Duration::from_secs(0))
                    }
                };
                if let Some(delay) = retry_sn_list {
                    if future::timeout(delay, self.wait_establish()).await.is_err() {
                        if let Some(sn_list) = build_params.retry_sn_list(&stack, &sn) {
                            info!("{} retry sn list call, sn={:?}", self, sn_list);
                            let _ = self.call_sn(sn_list, first_box).await;
                        }
                    }
                }
            }
        } 

        Ok(())
    }

    pub async fn build(&self) {
        self.sync_tunnel_state();
        let _ = self.build_inner().await.
            map_err(|err| {
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
            });
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

    async fn call_sn(&self, sn_list: Vec<DeviceId>, first_box: Arc<PackageBox>) -> BuckyResult<()> {
        let (cancel, reg) = AbortHandle::new_pair();

        let builder = self.clone();
        task::spawn(async move {
            let _ = builder.wait_establish().await;
            cancel.abort();
        });

        let (sender, receiver) = async_std::channel::bounded::<BuckyResult<()>>(1);
        let builder = self.clone();
        task::spawn(async move {
            let result = Abortable::new(builder.call_sn_inner(sn_list.clone(), first_box), reg).await;
            let result = match result {
                Ok(result) => result, 
                Err(_) => {
                    info!("{} call sn interrupted, sn={:?}", builder, sn_list);
                    Err(BuckyError::new(BuckyErrorCode::Interrupted, "canceled"))
                }
            };
            let _ = sender.try_send(result);
        });
       
        receiver.recv().await.unwrap()
    }

    async fn call_sn_inner(&self, sn_list: Vec<DeviceId>, first_box: Arc<PackageBox>) -> BuckyResult<()> {
        let stack = Stack::from(&self.0.stack);
        let tunnel = &self.0.tunnel;
        let call_session = stack.sn_client().call().call(
            None,
            tunnel.remote(),
            &sn_list, 
            |sn_call| {
                let mut context = udp::PackageBoxEncodeContext::from(sn_call);
                //FIXME 先不调用raw_measure_with_context
                //let len = first_box.raw_measure_with_context(&mut context).unwrap();
                let mut buf = vec![0u8; MTU_LARGE];
                let b = first_box.raw_encode_with_context(&mut buf, &mut context, &None).unwrap();
                //buf[0..b.len()].as_ref()
                let len = MTU_LARGE - b.len();
                buf.truncate(len);
                info!("{} encode first box to sn call, len: {}, package_box {:?}", self, len, first_box);
                buf
            }).await.map_err(|err| {
                error!("{} call sn failed, sn={:?}, err={}", self, sn_list, err);
                err
            })?; 
        
        let mut success = false;
        loop {
            if let Some(session) = call_session.next().await
                .map_err(|err| {error!("{} call sn failed, sn={:?}, err={}", self, sn_list, err); err})
                .ok().and_then(|opt| opt) {
                match session.result().unwrap() {
                    Ok(remote) => {
                        if let Some(proxy_buidler) = {
                            info!("{} call sn session responsed, sn={:?}, endpoints={:?}", self, session.sn(), remote.connect_info().endpoints());
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
                                    debug!("{} ignore proxy builder for not in connecting state", self);
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

                        success = true;
                        let _ = self.explore_endpoint_pair(&remote, first_box.clone(), |_| true);
                    },
                    Err(err) => {
                        error!("{} call sn session failed, sn={:?}, err={}", self, session.sn(), err);
                    }
                }
            } else {
                break;
            }
        }
        
        if success {
            Ok(())
        } else {
            error!("{} call sn session failed, sn={:?}", self, sn_list);
            Err(BuckyError::new(BuckyErrorCode::Failed, "all failed"))
        }
    }

    fn explore_endpoint_pair<F: Fn(&Endpoint) -> bool>(&self, remote: &Device, first_box: Arc<PackageBox>, filter: F) -> Vec<DynBuildTunnelAction> {
        let stack = Stack::from(&self.0.stack);
        let tunnel = &self.0.tunnel;
        let net_listener = stack.net_manager().listener();

        let mut actions = vec![];

        let connect_info = remote.connect_info();
        
        // FIXME: ipv6 udp frame may not support supper frame, simply ignore it now
        for udp_interface in net_listener.udp().iter().filter(|ui| ui.local().addr().is_ipv4()) {
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_udp() && ep.is_same_ip_version(&udp_interface.local()) && filter(ep)) {
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
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_tcp() && filter(ep)) {
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
            let first_box = builder.first_box(&stack.sn_client().ping().default_local()).await;
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




