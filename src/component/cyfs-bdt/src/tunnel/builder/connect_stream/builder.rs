use log::*;
use std::{
    fmt, 
    //time::Duration,
    sync::RwLock, 
    collections::BTreeMap
};
use async_std::{sync::{Arc}, task};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::*, 
    interface::*, 
    sn::client::PingClientCalledEvent, 
    stack::{WeakStack, Stack}, 
    stream::{StreamContainer}
};
use super::super::super::{BuildTunnelParams, tunnel::ProxyType};
use super::super::{action::*, builder::*, proxy::*};
use super::{action::*, package::*, tcp::*};


struct Connecting1State { 
    action_entries: BTreeMap<EndpointPair, DynConnectStreamAction>, 
    proxy: Option<ProxyBuilder>, 
    waiter: StateWaiter 
}

impl Connecting1State {
    fn add_action<T: ConnectStreamAction>(&mut self, action: T) -> Result<DynConnectStreamAction, BuckyError> {
        self.add_dyn_action(action.clone_as_connect_stream_action())
    }

    fn add_dyn_action(&mut self, action: DynConnectStreamAction) -> Result<DynConnectStreamAction, BuckyError> {
        let ep_pair = EndpointPair::from((*action.local(), *action.remote()));
        if self.action_entries.get(&ep_pair).is_some() {
            Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "action on endpoint pair exists"))
        } else {
            let _ = self.action_entries.insert(ep_pair, action.clone_as_connect_stream_action());
            Ok(action)
        }
    }

    fn action_of(&self, ep_pair: &EndpointPair) -> Option<DynConnectStreamAction> {
        self.action_entries.get(ep_pair).map(|a| a.clone_as_connect_stream_action())
    }
}

struct Connecting2State {
    action: DynConnectStreamAction, 
    waiter: StateWaiter
}


enum ConnectStreamBuilderState {
    Connecting1(Connecting1State), 
    Connecting2(Connecting2State),  
    Establish, 
    Closed
}
struct ConnectStreamBuilderImpl {
    stack: WeakStack, 
    params: BuildTunnelParams, 
    stream: StreamContainer, 
    state: RwLock<ConnectStreamBuilderState>
}

#[derive(Clone)]
pub struct ConnectStreamBuilder(Arc<ConnectStreamBuilderImpl>);

impl fmt::Display for ConnectStreamBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConnectStreamBuilder{{stream:{}}}", self.0.stream.as_ref())
    }
}

impl ConnectStreamBuilder {
    pub fn new(
        stack: WeakStack, 
        params: BuildTunnelParams, 
        stream: StreamContainer) -> Self {
        Self(Arc::new(ConnectStreamBuilderImpl {
            stack, 
            params, 
            stream,
            state: RwLock::new(ConnectStreamBuilderState::Connecting1(Connecting1State {
                action_entries: BTreeMap::new(), 
                proxy: None, 
                waiter: StateWaiter::new()
            }))
        }))
    }

    pub async fn build(&self) {
        self.sync_state_with_stream();
        let stack = Stack::from(&self.0.stack);
        let stream = &self.0.stream;
        let local = stack.local().clone();
        let build_params = &self.0.params;
        
        let first_box = self.first_box(&local).await;
        if first_box.is_none() {
            return ;
        }

        let first_box = Arc::new(first_box.unwrap());

        let actions = if let Some(remote) = build_params.remote_desc.as_ref() {
            self.explore_endpoint_pair(remote, first_box.clone(), |ep| ep.is_static_wan())
        } else {
            vec![]
        };
        
        if actions.len() == 0 {
            if build_params.remote_sn.len() == 0 {
                let _ = stream.as_ref().cancel_connecting_with(&BuckyError::new(BuckyErrorCode::InvalidParam, "neither remote device nor sn in build params"));
                return;
            } 
            if let Some(sn) = stack.device_cache().get(&build_params.remote_sn[0]).await {
                match self.call_sn(sn, first_box).await {
                    Ok(actions) => {
                        if actions.len() == 0 {
                            let _ = stream.as_ref().cancel_connecting_with(&BuckyError::new(BuckyErrorCode::NotConnected, "on endpoint pair can establish"));
                        }
                    },
                    Err(err) => {
                        let msg = format!("call sn err:{}", err.msg());
                        let _ = stream.as_ref().cancel_connecting_with(&BuckyError::new(err.code(), msg.as_str()));
                    }
                }
            } else {
                let _ = stream.as_ref().cancel_connecting_with(&BuckyError::new(BuckyErrorCode::InvalidParam, "got sn device object failed"));
            } 
        }
    }

    async fn first_box(&self, local: &Device) -> Option<PackageBox> {
        let stream = &self.0.stream;
        let stack = Stack::from(&self.0.stack);

        let syn_session_data = stream.as_ref().syn_session_data();
        if syn_session_data.is_none() {
            return None;
        }
        let syn_session_data = syn_session_data.unwrap();
        let key_stub = stack.keystore().create_key(stream.as_ref().tunnel().remote(), true);
        // ???????????????package box
        let mut first_box = PackageBox::encrypt_box(stream.as_ref().tunnel().remote().clone(), key_stub.aes_key.clone());
            
        let syn_tunnel = SynTunnel {
            from_device_id: local.desc().device_id(), 
            to_device_id: stream.as_ref().tunnel().remote().clone(), 
            from_container_id: IncreaseId::default(),
            from_device_desc: local.clone(),
            sequence: syn_session_data.syn_info.as_ref().unwrap().sequence.clone(), 
            send_time: syn_session_data.send_time.clone()
        };
        if !key_stub.is_confirmed {
            let mut exchg = Exchange::from(&syn_tunnel);
            let _ = exchg.sign(&key_stub.aes_key, stack.keystore().signer()).await;
            first_box.push(exchg);
        }
        first_box.push(syn_tunnel).push(syn_session_data.clone());
        Some(first_box)
    }

    async fn call_sn(&self, sn: Device, first_box: Arc<PackageBox>) -> Result<Vec<DynConnectStreamAction>, BuckyError> {
        let stack = Stack::from(&self.0.stack);
        let stream = &self.0.stream;
        let tunnel = stream.as_ref().tunnel();
        let remote = stack.sn_client().call(
            &vec![],  
            tunnel.remote(),
            &sn, 
            true, 
            true,
            false,
            |sn_call| {
                let mut context = udp::PackageBoxEncodeContext::from((tunnel.remote_const(), sn_call));
                //FIXME ????????????raw_measure_with_context
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
                ConnectStreamBuilderState::Connecting1(connecting1) => {
                    if connecting1.proxy.is_none() {
                        let proxy = ProxyBuilder::new(
                            tunnel.clone(), 
                            remote.get_obj_update_time(),  
                            first_box.clone());
                        debug!("{} create proxy builder", self);
                        connecting1.proxy = Some(proxy);
                    }
                    connecting1.proxy.clone()
                }, 
                _ => {
                    debug!("{} ignore proxy builder for not in connecting1 state", self);
                    None
                }
            }
        } {
            //FIXME: ???????????????proxy??????
            for proxy in stack.proxy_manager().active_proxies() {
                let _ = proxy_buidler.syn_proxy(ProxyType::Active(proxy)).await;
            }
            for proxy in remote.connect_info().passive_pn_list().iter().cloned() {
                let _ = proxy_buidler.syn_proxy(ProxyType::Passive(proxy)).await;
            }
        }
        
        Ok(self.explore_endpoint_pair(&remote, first_box, |_| true))
    }

    fn explore_endpoint_pair<F: Fn(&Endpoint) -> bool>(&self, remote: &Device, first_box: Arc<PackageBox>, filter: F) -> Vec<DynConnectStreamAction> {
        let stack = Stack::from(&self.0.stack);
        let stream = &self.0.stream;
        let net_listener = stack.net_manager().listener();

        let mut has_udp_tunnel = false;
        let mut actions = vec![];

        let connect_info = remote.connect_info();
        for udp_interface in net_listener.udp() {
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_udp() && ep.is_same_ip_version(&udp_interface.local()) && filter(ep)) {
                if let Ok(tunnel) = stream.as_ref().tunnel().create_tunnel(EndpointPair::from((udp_interface.local(), *remote_ep)), ProxyType::None) {
                    SynUdpTunnel::new(
                        tunnel, 
                        first_box.clone(), 
                        stream.as_ref().tunnel().config().udp.holepunch_interval); 
                    has_udp_tunnel = true; 
                }
            }
        }

        // for local_ip in net_listener.ip_set() {
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_tcp() && filter(ep)) {
                if let Ok(tunnel) = stream.as_ref().tunnel().create_tunnel(EndpointPair::from((Endpoint::default_tcp(remote_ep), *remote_ep)), ProxyType::None) {
                    let action = ConnectTcpStream::new(
                        self.0.stack.clone(), 
                        self.0.stream.clone(), 
                        tunnel
                    );
                    actions.push(action.clone_as_connect_stream_action());
                    self.wait_action_pre_establish(action);  
                }
            }
        // }

        if has_udp_tunnel {
            let action = ConnectPackageStream::new(self.0.stream.clone());
            action.begin();
            actions.push(action.clone_as_connect_stream_action());
            self.wait_action_pre_establish(action);
        }

        match &mut *self.0.state.write().unwrap() {
            ConnectStreamBuilderState::Connecting1(ref mut connecting1) => {
                for a in &actions {
                    let _ = connecting1.add_dyn_action(a.clone_as_connect_stream_action());
                }
            }, 
            _ => {
                // do nothing
            }
        }

        actions
    }  

    fn sync_state_with_stream(&self) {
        // ?????? stream ???establish??????
        // ???stream ???wait establish ????????????builder ??????establish ?????? closed??????
        let builder = self.clone();
        task::spawn(async move {
            let builder_impl = &builder.0;
            let waiter = match builder_impl.stream.as_ref().wait_establish().await {
                Ok(_) => {
                    let state = &mut *builder_impl.state.write().unwrap();
                    match state {
                        ConnectStreamBuilderState::Connecting1(_) => {
                            unreachable!("connection never establish when builder still connecting1")
                        }, 
                        ConnectStreamBuilderState::Connecting2(ref mut connecting2) => {
                            info!("{} connecting2 => establish", builder);
                            let waiter = connecting2.waiter.transfer();
                            *state = ConnectStreamBuilderState::Establish;
                            waiter
                        }, 
                        ConnectStreamBuilderState::Establish => {
                            unreachable!("connection never establish when builder has established")
                        }, 
                        ConnectStreamBuilderState::Closed => {
                            // do nothing
                            // ??????????????????builder??????????????????stream ??????????????????, ????????????
                            unreachable!("connection never establish when builder has closed")
                        }
                    }
                },
                Err(_) => {
                    let state = &mut *builder_impl.state.write().unwrap();
                    match state {
                        ConnectStreamBuilderState::Connecting1(ref mut connecting1) => {
                            info!("{} connecting1 => closed", builder);
                            let waiter = connecting1.waiter.transfer();
                            *state = ConnectStreamBuilderState::Closed;
                            waiter
                        }, 
                        ConnectStreamBuilderState::Connecting2(ref mut connecting2) => {
                            info!("{} connecting2 => closed", builder);
                            let waiter = connecting2.waiter.transfer();
                            *state = ConnectStreamBuilderState::Closed;
                            waiter
                        }, 
                        ConnectStreamBuilderState::Establish => {
                            unreachable!("connection never close when builder has established")
                        }, 
                        ConnectStreamBuilderState::Closed => {
                            unreachable!("connection never close when builder has closed")
                        }
                    }
                }
            };
            
            waiter.wake();
        });
    }

    fn wait_action_pre_establish<T: 'static + ConnectStreamAction>(&self, action: T) {
        // ?????????action??????establish ??????????????????action???builder??????pre establish??? ?????? continue connect
        let builder = self.clone();
        task::spawn(async move {
            let continue_action = match action.wait_pre_establish().await {
                ConnectStreamState::PreEstablish => {
                    let state = &mut *builder.0.state.write().unwrap();
                    match state {
                        ConnectStreamBuilderState::Connecting1(ref mut connecting1) => {
                            info!("{} connecting1 => connecting2 use action {}", builder, action);
                            let connecting2 = Connecting2State {
                                waiter: connecting1.waiter.transfer(), 
                                action: action.clone_as_connect_stream_action()
                            };
                            *state = ConnectStreamBuilderState::Connecting2(connecting2);
                            Some(action.clone_as_connect_stream_action())
                        }, 
                        _ => {
                            None
                        }
                    }
                },  
                _ => {
                    None
                }
            };
            if let Some(continue_action) = continue_action {
                let _ = continue_action.continue_connect().await;
            }
        });
    }
}

#[async_trait]
impl TunnelBuilder for ConnectStreamBuilder {
    fn sequence(&self) -> TempSeq {
        self.0.stream.sequence()
    }

    fn state(&self) -> TunnelBuilderState {
        match &*self.0.state.read().unwrap() {
            ConnectStreamBuilderState::Connecting1(_) => TunnelBuilderState::Connecting, 
            ConnectStreamBuilderState::Connecting2(_) => TunnelBuilderState::Connecting,
            ConnectStreamBuilderState::Establish => TunnelBuilderState::Establish,
            ConnectStreamBuilderState::Closed => TunnelBuilderState::Closed,
        }
    }

    async fn wait_establish(&self) -> Result<(), BuckyError> {
        let (state, waiter) = match &mut *self.0.state.write().unwrap() {
            ConnectStreamBuilderState::Connecting1(connecting1) => (TunnelBuilderState::Connecting, Some(connecting1.waiter.new_waiter())),
            ConnectStreamBuilderState::Connecting2(connecting2) => (TunnelBuilderState::Connecting, Some(connecting2.waiter.new_waiter())),
            ConnectStreamBuilderState::Establish => (TunnelBuilderState::Establish, None),
            ConnectStreamBuilderState::Closed => (TunnelBuilderState::Closed, None)
        };
        match {
            if let Some(waiter) = waiter {
                StateWaiter::wait(waiter, | | self.state()).await
            } else {
                state
            }
        } {
            TunnelBuilderState::Establish => Ok(()), 
            TunnelBuilderState::Closed => Err(BuckyError::new(BuckyErrorCode::Failed, "builder failed")),
            _ => unreachable!()
        }
    }
}

impl OnPackage<TcpAckConnection, tcp::AcceptInterface> for ConnectStreamBuilder {
    fn on_package(&self, pkg: &TcpAckConnection, interface: tcp::AcceptInterface) -> Result<OnPackageResult, BuckyError> {
        debug!("{} on package {} from {}", self, pkg, interface);
        // ??????????????? ep pair???????????????tcp stream?????????????????????
        let action = AcceptReverseTcpStream::new(self.0.stream.clone(), *interface.local(), *interface.remote());
        
        let _ = match &mut *self.0.state.write().unwrap() {
            ConnectStreamBuilderState::Connecting1(ref mut connecting1) => {
                connecting1.add_action(action.clone())
            }, 
            _ => {
                Err(BuckyError::new(BuckyErrorCode::ErrorState, "accept tcp interface with tcp ack connection in no connecting1 state"))
            }
        }.map_err(|err| {
            debug!("{} ingore tcp ack connection for not in connecting1", self);
            err
        })?;
        self.wait_action_pre_establish(action.clone());
        action.on_package(pkg, interface)
    }
}

impl OnPackage<SessionData> for ConnectStreamBuilder {
    fn on_package(&self, pkg: &SessionData, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        if pkg.is_syn() {
            unreachable!()
        } else if pkg.is_syn_ack() {
            let action = match &*self.0.state.read().unwrap() {
                ConnectStreamBuilderState::Connecting1(ref connecting1) => {
                    connecting1.action_of(&ConnectPackageStream::endpoint_pair()).map(|a| ConnectPackageStream::from(a)).ok_or_else(| | BuckyError::new(BuckyErrorCode::ErrorState, "got syn ack while package stream not connecting"))
                }, 
                _ => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "got syn ack  in no connecting1 state"))
                }
            }?;
            action.on_package(pkg, None)
        } else {
            unreachable!()
        }
    }
}

impl PingClientCalledEvent for ConnectStreamBuilder {
    fn on_called(&self, called: &SnCalled, _context: ()) -> Result<(), BuckyError> {
        let builder = self.clone();
        let active_pn_list = called.active_pn_list.clone();
        let remote_timestamp = called.peer_info.get_obj_update_time();
        task::spawn(async move {
            let stack = Stack::from(&builder.0.stack);
            let tunnel = builder.0.stream.as_ref().tunnel().clone();
            if let Some(first_box) = builder.first_box(&stack.device_cache().local()).await {
                if let Some(proxy_builder) = {
                    let state = &mut *builder.0.state.write().unwrap();
                    match state {
                        ConnectStreamBuilderState::Connecting1(connecting1) => {
                            if connecting1.proxy.is_none() {
                                let proxy = ProxyBuilder::new(
                                    tunnel, 
                                    remote_timestamp,  
                                    Arc::new(first_box));
                                debug!("{} create proxy builder", builder);
                                connecting1.proxy = Some(proxy);
                            }
                            
                            connecting1.proxy.clone()
                        }, 
                        _ => None
                    }
                } {
                    //FIXME: ???????????????proxy??????
                    for proxy in active_pn_list {
                        let _ = proxy_builder.syn_proxy(ProxyType::Active(proxy));
                    }
                }
            }
        });
        
        Ok(())
    }
}

impl OnPackage<AckProxy, &DeviceId> for ConnectStreamBuilder {
    fn on_package(&self, pkg: &AckProxy, proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> {
        if let Some(proxy_builder) = match &*self.0.state.read().unwrap() {
            ConnectStreamBuilderState::Connecting1(connecting1) => {
                connecting1.proxy.clone()
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