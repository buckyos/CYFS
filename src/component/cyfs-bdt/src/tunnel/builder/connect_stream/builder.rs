use log::*;
use std::{
    fmt, 
    //time::Duration,
    sync::RwLock, 
    collections::{BTreeMap, LinkedList}
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
    stack::{WeakStack, Stack}, 
    stream::{StreamContainer}
};
use super::super::super::{BuildTunnelParams, tunnel::ProxyType};
use super::super::{action::*, builder::*, proxy::*};
use super::{action::*, package::*, tcp::*};


struct ConnectingState { 
    action_entries: BTreeMap<EndpointPair, DynConnectStreamAction>, 
    pre_established_actions: LinkedList<DynConnectStreamAction>,     
    proxy: Option<ProxyBuilder>, 
    waiter: StateWaiter 
}

impl ConnectingState {
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



enum ConnectStreamBuilderState {
    Connecting(ConnectingState), 
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
            state: RwLock::new(ConnectStreamBuilderState::Connecting(ConnectingState {
                action_entries: BTreeMap::new(), 
                pre_established_actions: LinkedList::new(), 
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
        let key_stub = stack.keystore().create_key(stream.as_ref().tunnel().remote_const(), true);
        // 生成第一个package box
        let mut first_box = PackageBox::encrypt_box(
            stream.as_ref().tunnel().remote().clone(), 
            key_stub.key.clone());
            
        let syn_tunnel = SynTunnel {
            protocol_version: stream.as_ref().tunnel().protocol_version(), 
            stack_version: stream.as_ref().tunnel().stack_version(), 
            to_device_id: stream.as_ref().tunnel().remote().clone(), 
            from_device_desc: local.clone(),
            sequence: syn_session_data.syn_info.as_ref().unwrap().sequence.clone(), 
            send_time: syn_session_data.send_time.clone()
        };
        if let keystore::EncryptedKey::Unconfirmed(encrypted) = key_stub.encrypted {
            let mut exchg = Exchange::from((&syn_tunnel, encrypted, key_stub.key.mix_key));
            let _ = exchg.sign(stack.keystore().signer()).await;
            first_box.push(exchg);
        }
        first_box.push(syn_tunnel).push(syn_session_data.clone_with_data());
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
                let mut context = udp::PackageBoxEncodeContext::from(sn_call);
                //FIXME 先不调用raw_measure_with_context
                //let len = first_box.raw_measure_with_context(&mut context).unwrap();
                let mut buf = vec![0u8; 2048];
                let b = first_box.raw_encode_with_context(&mut buf, &mut context, &None).unwrap();
                //buf[0..b.len()].as_ref()
                let len = 2048 - b.len();
                buf.truncate(len);
                info!("{} encode first box to sn call, len: {}, package_box {:?}", self, len, first_box);
                buf
            }).await?;
        
        if let Some(proxy_buidler) = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                ConnectStreamBuilderState::Connecting(connecting) => {
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
            ConnectStreamBuilderState::Connecting(ref mut connecting) => {
                for a in &actions {
                    let _ = connecting.add_dyn_action(a.clone_as_connect_stream_action());
                }
            }, 
            _ => {
                // do nothing
            }
        }

        actions
    }  

    fn sync_state_with_stream(&self) {
        // 同步 stream 的establish状态
        // 当stream 的wait establish 返回时，builder 进入establish 或者 closed状态
        let builder = self.clone();
        task::spawn(async move {
            let builder_impl = &builder.0;
            let waiter = match builder_impl.stream.as_ref().wait_establish().await {
                Ok(_) => {
                    let state = &mut *builder_impl.state.write().unwrap();
                    match state {
                        ConnectStreamBuilderState::Connecting(ref mut connecting) => {
                            info!("{} connecting => establish", builder);
                            let waiter = connecting.waiter.transfer();
                            *state = ConnectStreamBuilderState::Establish;
                            waiter
                        }, 
                        ConnectStreamBuilderState::Establish => {
                            unreachable!("connection never establish when builder has established")
                        }, 
                        ConnectStreamBuilderState::Closed => {
                            // do nothing
                            // 时序上确实有builder先出错，但是stream 联通了的情况, 忽略就好
                            unreachable!("connection never establish when builder has closed")
                        }
                    }
                },
                Err(_) => {
                    let state = &mut *builder_impl.state.write().unwrap();
                    match state {
                        ConnectStreamBuilderState::Connecting(ref mut connecting) => {
                            info!("{} connecting1 => closed", builder);
                            let waiter = connecting.waiter.transfer();
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

    fn stream(&self) -> &StreamContainer {
        &self.0.stream
    }

    fn wait_action_pre_establish<T: 'static + ConnectStreamAction>(&self, action: T) {
        // 第一个action进入establish 时，忽略其他action，builder进入pre establish， 调用 continue connect
        let builder = self.clone();
        task::spawn(async move {
            let action = match action.wait_pre_establish().await {
                ConnectStreamState::PreEstablish => {
                    let state = &mut *builder.0.state.write().unwrap();
                    match state {
                        ConnectStreamBuilderState::Connecting(ref mut connecting) => {
                            connecting.pre_established_actions.push_back(action.clone_as_connect_stream_action());
                            if connecting.pre_established_actions.len() == 1 {
                                connecting.pre_established_actions.front().map(|a| a.clone_as_connect_stream_action())
                            } else {
                                None
                            }
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

            
            if let Some(action) = action {
                let mut action = action;
                loop {
                    match action.continue_connect().await {
                        Ok(selector) => {
                            let _ = builder.stream().as_ref().establish_with(selector, builder.stream()).await;
                            break;
                        }, 
                        Err(_) => {
                            let state = &mut *builder.0.state.write().unwrap();
                            match state {
                                ConnectStreamBuilderState::Connecting(ref mut connecting) => {
                                    let _ = connecting.pre_established_actions.pop_front().unwrap();
                                    if let Some(next) = connecting.pre_established_actions.front() {
                                        action = next.clone_as_connect_stream_action();
                                    } else {
                                        break;    
                                    }
                                }
                                _ => {
                                    break;
                                }
                            }
                        }
                    }
                }
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
            ConnectStreamBuilderState::Connecting(_) => TunnelBuilderState::Connecting, 
            ConnectStreamBuilderState::Establish => TunnelBuilderState::Establish,
            ConnectStreamBuilderState::Closed => TunnelBuilderState::Closed,
        }
    }

    async fn wait_establish(&self) -> Result<(), BuckyError> {
        let (state, waiter) = match &mut *self.0.state.write().unwrap() {
            ConnectStreamBuilderState::Connecting(connecting) => (TunnelBuilderState::Connecting, Some(connecting.waiter.new_waiter())),
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
        // 如果以相同 ep pair发起了多条tcp stream，只保留第一条
        let action = AcceptReverseTcpStream::new(self.0.stream.clone(), *interface.local(), *interface.remote());
        
        let _ = match &mut *self.0.state.write().unwrap() {
            ConnectStreamBuilderState::Connecting(ref mut connecting) => {
                connecting.add_action(action.clone())
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
                ConnectStreamBuilderState::Connecting(ref connecting) => {
                    connecting.action_of(&ConnectPackageStream::endpoint_pair()).map(|a| ConnectPackageStream::from(a)).ok_or_else(| | BuckyError::new(BuckyErrorCode::ErrorState, "got syn ack while package stream not connecting"))
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
                        ConnectStreamBuilderState::Connecting(connecting) => {
                            if connecting.proxy.is_none() {
                                let proxy = ProxyBuilder::new(
                                    tunnel, 
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
            }
        });
        
        Ok(())
    }
}

impl OnPackage<AckProxy, &DeviceId> for ConnectStreamBuilder {
    fn on_package(&self, pkg: &AckProxy, proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> {
        if let Some(proxy_builder) = match &*self.0.state.read().unwrap() {
            ConnectStreamBuilderState::Connecting(connecting) => {
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