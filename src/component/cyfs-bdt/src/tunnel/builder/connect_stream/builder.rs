use log::*;
use std::{
    fmt, 
    //time::Duration,
    sync::RwLock, 
    collections::{BTreeMap, LinkedList}, 
    time::Duration
};
use async_std::{
    sync::{Arc}, 
    task, 
    future
};
use async_trait::{async_trait};
use futures::future::{Abortable, AbortHandle};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::{*, udp::MTU_LARGE}, 
    history::keystore, 
    sn::client::PingClientCalledEvent, 
    stack::{WeakStack, Stack}, 
    tunnel::{TunnelContainer}, 
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
    start_at: Timestamp, 
    params: BuildTunnelParams, 
    stream: StreamContainer, 
    tunnel: TunnelContainer, 
    state: RwLock<ConnectStreamBuilderState>
}

#[derive(Clone)]
pub struct ConnectStreamBuilder(Arc<ConnectStreamBuilderImpl>);

impl fmt::Display for ConnectStreamBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConnectStreamBuilder{{stream:{}}}", self.0.stream)
    }
}

impl ConnectStreamBuilder {
    pub fn new(
        stack: WeakStack, 
        params: BuildTunnelParams, 
        stream: StreamContainer, 
        tunnel: TunnelContainer) -> Self {
        Self(Arc::new(ConnectStreamBuilderImpl {
            stack, 
            params, 
            start_at: bucky_time_now(), 
            stream, 
            tunnel, 
            state: RwLock::new(ConnectStreamBuilderState::Connecting(ConnectingState {
                action_entries: BTreeMap::new(), 
                pre_established_actions: LinkedList::new(), 
                proxy: None, 
                waiter: StateWaiter::new()
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

    fn tunnel(&self) -> &TunnelContainer {
        &self.0.tunnel
    }

    async fn build_inner(&self) -> BuckyResult<()> {
        let stack = Stack::from(&self.0.stack);

        let local = stack.sn_client().ping().default_local();
        let build_params = &self.0.params;
        
        let first_box = self.first_box(&local).await;
        if first_box.is_none() {
            return Ok(());
        }

        let first_box = Arc::new(first_box.unwrap());

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
        self.sync_state_with_stream();

        let _ = self.build_inner().await
            .map_err(|err| {
                let _ = self.0.stream.cancel_connecting_with(&err);
            });
    }

    async fn first_box(&self, local: &Device) -> Option<PackageBox> {
        let stream = &self.0.stream;
        let stack = Stack::from(&self.0.stack);

        let syn_session_data = stream.syn_session_data();
        if syn_session_data.is_none() {
            return None;
        }
        let syn_session_data = syn_session_data.unwrap();
        let key_stub = stack.keystore().create_key(self.tunnel().remote_const(), true);
        // 生成第一个package box
        let mut first_box = PackageBox::encrypt_box(
            self.tunnel().remote().clone(), 
            key_stub.key.clone());
            
        let syn_tunnel = SynTunnel {
            protocol_version: self.tunnel().protocol_version(), 
            stack_version: self.tunnel().stack_version(), 
            to_device_id: self.tunnel().remote().clone(),
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
        let call_session = stack.sn_client().call().call(
            None,
            self.tunnel().remote(),
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
                        info!("{} call sn session responsed, sn={:?}, endpoints={:?}", self, session.sn(), remote.connect_info().endpoints());
                        if let Some(proxy_buidler) = {
                            let state = &mut *self.0.state.write().unwrap();
                            match state {
                                ConnectStreamBuilderState::Connecting(connecting) => {
                                    if connecting.proxy.is_none() {
                                        let proxy = ProxyBuilder::new(
                                            self.tunnel().clone(), 
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

    fn explore_endpoint_pair<F: Fn(&Endpoint) -> bool>(&self, remote: &Device, first_box: Arc<PackageBox>, filter: F) -> Vec<DynConnectStreamAction> {
        let stack = Stack::from(&self.0.stack);
        let net_listener = stack.net_manager().listener();

        let mut has_udp_tunnel = false;
        let mut actions = vec![];

        let connect_info = remote.connect_info();
        for udp_interface in net_listener.udp() {
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_udp() && ep.is_same_ip_version(&udp_interface.local()) && filter(ep)) {
                if let Ok((tunnel, newly_created)) = self.tunnel().create_tunnel(EndpointPair::from((udp_interface.local(), *remote_ep)), ProxyType::None) {
                    if newly_created {
                        SynUdpTunnel::new(
                            tunnel, 
                            first_box.clone(),
                            self.tunnel().config().udp.holepunch_interval);
                        has_udp_tunnel = true; 
                    }
                }
            }
        }

        // for local_ip in net_listener.ip_set() {
            for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_tcp() && filter(ep)) {
                if let Ok((tunnel, newly_created)) = self.tunnel().create_tunnel(EndpointPair::from((Endpoint::default_tcp(remote_ep), *remote_ep)), ProxyType::None) {
                    if newly_created {
                        let action = ConnectTcpStream::new(
                            self.0.stack.clone(), 
                            self.0.stream.clone(), 
                            tunnel
                        );
                        actions.push(action.clone_as_connect_stream_action());
                        self.wait_action_pre_establish(action);  
                    }
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
            let waiter = match builder_impl.stream.wait_establish().await {
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
                            let _ = builder.stream().establish_with(selector).await;
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
            let tunnel = builder.tunnel().clone();
            if let Some(first_box) = builder.first_box(&stack.sn_client().ping().default_local()).await {
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