use log::*;
use std::{
    time::Duration, 
    sync::RwLock, 
    fmt,
    convert::TryFrom
};
use async_std::{task, sync::{Arc, Weak}, future};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::*, 
    sn::client::PingClientCalledEvent, 
    stack::{WeakStack, Stack}, 
    tunnel::{self, Tunnel, ProxyType},
    stream::{StreamContainer, StreamProviderSelector}
};
use super::{
    action::*, 
    builder::*, 
    proxy::ProxyBuilder
};



struct AcceptPackageStreamImpl {
    builder: WeakAcceptStreamBuilder, 
    remote_id: IncreaseId
}

// 靠编译器推导层次太多
unsafe impl Send for AcceptPackageStreamImpl {}
unsafe impl Sync for AcceptPackageStreamImpl {}

#[derive(Clone)]
struct AcceptPackageStream(Arc<AcceptPackageStreamImpl>);

impl fmt::Display for AcceptPackageStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(builder) = AcceptStreamBuilder::try_from(&self.0.builder) {
            write!(f, "AcceptPackageStream{{stream:{}}}", builder.building_stream().as_ref())
        } else {
            write!(f, "AcceptPackageStream{{stream:{}}}", "unknown")
        }  
    }
}

impl AcceptPackageStream {
    fn new(
        builder: WeakAcceptStreamBuilder, 
        remote_id: IncreaseId, 
        resend_interval: Duration) -> Self {
        let a = Self(Arc::new(AcceptPackageStreamImpl {
            builder: builder.clone(), 
            remote_id, 
        }));

        task::spawn(async move { 
            if let Ok(builder) = AcceptStreamBuilder::try_from(&builder) {
                if let Ok(syn_ack) = builder.wait_confirm().await.map(|s| s.package_syn_ack.clone_with_data()) {
                    // 重发ack直到连接成功，因为ackack可能丢失
                    loop {
                        if !builder.building_stream().as_ref().is_connecting() {
                            break;
                        }
                        let packages = vec![DynamicPackage::from(syn_ack.clone_with_data())];
                        let _ = builder.building_stream().as_ref().tunnel().send_packages(packages);
                        future::timeout(resend_interval, future::pending::<()>()).await.err();
                    }
                }
            } else {
                error!("ignore resend ack loop for builder released");
            }
        });

        a
        
    }
    fn endpoint_pair() -> EndpointPair {
        EndpointPair::from((Endpoint::default(), Endpoint::default()))
    }
}

impl OnPackage<SessionData> for AcceptPackageStream {
    fn on_package(&self, pkg: &SessionData, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        if pkg.is_syn() {
            // 如果已经confirm了，立即回复ack
            if let Ok(builder) = AcceptStreamBuilder::try_from(&self.0.builder) {
                if let Some(syn_ack) = builder.confirm_syn_ack().map(|c| c.package_syn_ack.clone_with_data()) {
                    debug!("{} send session data with ack", self);
                    let packages = vec![DynamicPackage::from(syn_ack.clone_with_data())];
                    let _ = builder.building_stream().as_ref().tunnel().send_packages(packages);
                } else {
                    debug!("{} ingore syn session data for not confirmed", self);
                }
            } else {
                debug!("{} ingore syn session data for builder released", self);
            }
        } else if pkg.is_syn_ack() {
            unreachable!()
        } else {
            //没有syn标识的session data视为ackack,触发连接成功
            let action = self.clone();
            let pkg = pkg.clone_without_data();
            task::spawn(async move {
                if let Ok(builder) = AcceptStreamBuilder::try_from(&action.0.builder) {
                    let stream = builder.building_stream().clone();
                    let _ = stream.as_ref().establish_with(
                        StreamProviderSelector::Package(action.0.remote_id, Some(pkg)), 
                        &stream).await;
                } else {
                    debug!("{} ingore syn session data for {}", action, "builder released");
                }
                
            });
        }
        Ok(OnPackageResult::Handled)
    }
}



struct UnconfirmedState {
    waiter: StateWaiter
}

struct ConfirmSynAck {
    pub package_syn_ack: SessionData,
    pub tcp_syn_ack:  TcpAckConnection
}

struct ConfirmedState {
    syn_ack: Arc<ConfirmSynAck>
} 


enum ConfirmState {
    Unconfirmed(UnconfirmedState), 
    Confirmed(ConfirmedState),
}


struct ConnectingState {
    waiter: StateWaiter, 
    package_stream: Option<AcceptPackageStream>, 
    reverse_tcp: bool,  
    confirm_state: ConfirmState, 
    proxy: Option<ProxyBuilder>
}

enum AcceptStreamState {
    Connecting(ConnectingState), 
    Establish, 
    Closed
}

struct AcceptStreamBuilderImpl {
    stack: WeakStack, 
    stream: StreamContainer, 
    state: RwLock<AcceptStreamState>
}

#[derive(Clone)]
pub struct AcceptStreamBuilder(Arc<AcceptStreamBuilderImpl>);

#[derive(Clone)]
struct WeakAcceptStreamBuilder(Weak<AcceptStreamBuilderImpl>);

impl fmt::Display for AcceptStreamBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AcceptStreamBuilder{{stream:{}}}", self.building_stream().as_ref())
    }
}


impl AcceptStreamBuilder {
    pub fn new(
        stack: WeakStack, 
        stream: StreamContainer
    ) -> Self {
        let builder= Self(Arc::new(AcceptStreamBuilderImpl {
            stack, 
            stream, 
            state: RwLock::new(AcceptStreamState::Connecting(ConnectingState {
                waiter: StateWaiter::new(), 
                package_stream: None, 
                reverse_tcp: false, 
                confirm_state: ConfirmState::Unconfirmed(UnconfirmedState {
                    waiter: StateWaiter::new()
                }),
                proxy: None
            }))
        }));

        {
            // 同步 stream 的establish状态
            // 当stream 的wait establish 返回时，builder 进入establish 或者 closed状态
            let builder = builder.clone();
            task::spawn(async move {
                let builder_impl = &builder.0;
                let waiter = match builder_impl.stream.as_ref().wait_establish().await {
                    Ok(_) => {
                        let state = &mut *builder_impl.state.write().unwrap();
                        match state {
                            AcceptStreamState::Connecting(ref mut connecting) => {
                                info!("{} Connecting => Establish", builder);
                                let waiter = connecting.waiter.transfer();
                                *state = AcceptStreamState::Establish;
                                waiter
                            }, 
                            AcceptStreamState::Establish => {
                                unreachable!("connection never establish when builder has established")
                            }, 
                            AcceptStreamState::Closed => {
                                // do nothing
                                // 时序上确实有builder先出错，但是stream 联通了的情况, 忽略就好
                                unreachable!("connection never establish when builder has closed")
                            }
                        }
                    },
                    Err(_) => {
                        let state = &mut *builder_impl.state.write().unwrap();
                        match state {
                            AcceptStreamState::Connecting(ref mut connecting) => {
                                error!("{} Connecting => Closed", builder);
                                let mut waiter = connecting.waiter.transfer();
                                match &mut connecting.confirm_state {
                                    ConfirmState::Unconfirmed(unconfirmed) => {
                                        // 出错时还要唤醒 wait confirm
                                        unconfirmed.waiter.transfer_into(&mut waiter);
                                    }, 
                                    _ => {

                                    }
                                }
                                *state = AcceptStreamState::Closed;
                                waiter
                            }, 
                            AcceptStreamState::Establish => {
                                unreachable!("connection never close when builder has established")
                            }, 
                            AcceptStreamState::Closed => {
                                unreachable!("connection never close when builder has closed")
                            }
                        }
                    }
                };
                
                waiter.wake();
            });
        }

        builder
    }

    pub fn confirm(&self, answer: &[u8]) -> Result<(), BuckyError> {
        info!("{} confirm answer_len={}", self, answer.len());
        let confirm_ack = ConfirmSynAck {
            package_syn_ack: self.0.stream.as_ref().syn_ack_session_data(answer).ok_or_else(|| BuckyError::new(BuckyErrorCode::ErrorState, "stream not connecting"))?,
            tcp_syn_ack: self.0.stream.as_ref().ack_tcp_stream(answer).ok_or_else(|| BuckyError::new(BuckyErrorCode::ErrorState, "stream not connecting"))?
        };
        let waiter = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                AcceptStreamState::Connecting(connecting) => {
                    match connecting.confirm_state {
                        ConfirmState::Unconfirmed(ref mut unconfirm) => {
                            let waiter = unconfirm.waiter.transfer(); 
                            *&mut connecting.confirm_state = ConfirmState::Confirmed(ConfirmedState {
                                syn_ack: Arc::new(confirm_ack)});
                            Ok(waiter)
                        },
                        ConfirmState::Confirmed(_) => {
                            Err(BuckyError::new(BuckyErrorCode::ErrorState, "stream has confirmed"))
                        }
                    }
                },
                _ => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "stream not connecting"))
                }
            }
            
        }.map(|v| {info!("{} unconfirmed => Confirmed", self);v})
            .map_err(|e| {info!("{} ignore confirm for {}", self, e);e})?;
        
        waiter.wake();
        Ok(())
    }

    async fn build(&self, caller_box: PackageBox, active_pn_list: Vec<DeviceId>) -> Result<(), BuckyError> {
        info!("{} build with active pn {:?}", self, active_pn_list);
        {
            let stack = Stack::from(&self.0.stack);
            let local = stack.device_cache().local();
            let stream = &self.0.stream;
            let net_listener = stack.net_manager().listener();
            let key = caller_box.key().clone();
            let syn_tunnel: &SynTunnel = caller_box.packages_no_exchange()[0].as_ref();
            let connect_info = syn_tunnel.from_device_desc.connect_info();

            // for local_ip in net_listener.ip_set() {
                for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_tcp()) {
                    // let local_ip = *local_ip;
                    let remote_ep = *remote_ep;
                    let builder = self.clone();
                    let remote_constinfo = syn_tunnel.from_device_desc.desc().clone();
                    let remote_timestamp = syn_tunnel.from_device_desc.body().as_ref().unwrap().update_time();
                    let key = key.clone();
                    task::spawn(async move {
                        let _ = builder.reverse_tcp_stream(
                            /*local_ip, */
                            remote_ep, 
                            remote_constinfo.device_id(), 
                            remote_constinfo, 
                            remote_timestamp, 
                            key
                        ).await
                            .map_err(|e| {
                                debug!("{} reverse tcp stream to {} failed for {}", builder, remote_ep, e);
                                e
                            });
                    });
                }
            // }

            let confirm_ack = self.wait_confirm().await?;
           
            // first box 包含 ack tunnel 和 session data
            let tunnel = self.building_stream().as_ref().tunnel();
            let ack_tunnel = SynTunnel {
                protocol_version: tunnel.protocol_version(), 
                stack_version: tunnel.stack_version(), 
                to_device_id: syn_tunnel.from_device_desc.desc().device_id(),
                sequence: syn_tunnel.sequence,
                from_device_desc: local,
                send_time: 0
            };
            let mut first_box = PackageBox::encrypt_box(caller_box.remote().clone(), caller_box.key().clone());
            first_box.append(vec![DynamicPackage::from(ack_tunnel), DynamicPackage::from(confirm_ack.package_syn_ack.clone_with_data())]);
            let first_box = Arc::new(first_box);

            for udp_interface in net_listener.udp() {
                for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_udp() && ep.is_same_ip_version(&udp_interface.local())) {
                    if let Ok(tunnel) = stream.as_ref().tunnel().create_tunnel(EndpointPair::from((udp_interface.local(), *remote_ep)), ProxyType::None) {
                        SynUdpTunnel::new(
                            tunnel, 
                            first_box.clone(), 
                            stream.as_ref().tunnel().config().udp.holepunch_interval);      
                    }    
                }  
            }

            if let Some(proxy_builder) = {
                let state = &mut *self.0.state.write().unwrap();
                match state {
                    AcceptStreamState::Connecting(connecting) => {
                        if connecting.proxy.is_none() {
                            connecting.proxy = Some(ProxyBuilder::new(
                                stream.as_ref().tunnel().clone(), 
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

    async fn reverse_tcp_stream(&self, 
        /*local_ip: IpAddr, */
        remote_ep: Endpoint, 
        remote_device_id: DeviceId, 
        remote_device_desc: DeviceDesc, 
        remote_timestamp: Timestamp, 
        key: MixAesKey
    ) -> Result<(), BuckyError> {
        debug!("{} reverse tcp stream to {} {} connect tcp interface", self, remote_device_id, remote_ep);
        let stack: Stack = Stack::from(&self.0.stack);
        let stream = self.building_stream();
        let tunnel: tunnel::tcp::Tunnel = stream.as_ref().tunnel().create_tunnel(EndpointPair::from((Endpoint::default_tcp(&remote_ep), remote_ep)), ProxyType::None)
            .map_err(|err| { 
                debug!("{} reverse tcp stream to {} {} connect tcp interface failed for {}", self, remote_device_id, remote_ep, err);
                err
            })?;

        let tcp_interface = tcp::Interface::connect(
            /*local_ip, */
            remote_ep, 
            remote_device_id.clone(), 
            remote_device_desc, 
            key, 
            stack.config().tunnel.tcp.connect_timeout).await
            .map_err(|err| { 
                tunnel.mark_dead(tunnel.state());
                debug!("{} reverse tcp stream to {} {} connect tcp interface failed for {}", self, remote_device_id, remote_ep, err);
                err
            })?;
        let tcp_ack = self.wait_confirm().await.map(|ack| ack.tcp_syn_ack.clone())
            .map_err(|err| { 
                let _ = tunnel.connect_with_interface(tcp_interface.clone());
                debug!("{} reverse tcp stream to {} {} wait confirm failed for {}", self, remote_device_id, remote_ep, err);
                err
            })?;
        let resp_box = tcp_interface.confirm_connect(&stack, vec![DynamicPackage::from(tcp_ack)], stack.config().tunnel.tcp.confirm_timeout).await
            .map_err(|err| {
                tunnel.mark_dead(tunnel.state());
                err
            })?;
        
        let resp_packages = resp_box.packages_no_exchange();
        if resp_packages.len() != 1 || resp_packages[0].cmd_code() != PackageCmdCode::TcpAckAckConnection {
            tunnel.mark_dead(tunnel.state());
            error!("{} reverse tcp stream to {} {} got error resp", self, remote_device_id, remote_ep);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, "invalid response"));
        }
        let ack_ack: &TcpAckAckConnection = resp_packages[0].as_ref();
        let _ = tunnel.pre_active(remote_timestamp);

        match ack_ack.result {
            TCP_ACK_CONNECTION_RESULT_OK => {
                stream.as_ref().establish_with(
                    StreamProviderSelector::Tcp(
                        tcp_interface.socket().clone(), 
                        tcp_interface.key().clone(), 
                        None), 
                    stream).await
            }, 
            TCP_ACK_CONNECTION_RESULT_REFUSED => {
                // do nothing
                Err(BuckyError::new(BuckyErrorCode::Reject, "remote rejected"))
            }, 
            _ => {
                tunnel.mark_dead(tunnel.state());
                Err(BuckyError::new(BuckyErrorCode::InvalidData, "invalid response"))
            }
        }
    }

    fn confirm_syn_ack(&self) -> Option<Arc<ConfirmSynAck>> {
        match &*self.0.state.read().unwrap() {
            AcceptStreamState::Connecting(connecting) => {
                match &connecting.confirm_state {
                    ConfirmState::Confirmed(confirmed) => Some(confirmed.syn_ack.clone()), 
                    _ => None
                }
            }, 
            _ => None
        }
    }

    async fn wait_confirm(&self) -> Result<Arc<ConfirmSynAck>, BuckyError> {
        let (ack, waiter) = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                AcceptStreamState::Connecting(connecting) => {
                    match &mut connecting.confirm_state {
                        ConfirmState::Confirmed(confirmed) => Ok((Some(confirmed.syn_ack.clone()), None)), 
                        ConfirmState::Unconfirmed(unconfirmed) => Ok((None, Some(unconfirmed.waiter.new_waiter()))) 
                    }      
                },
                _ => {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "builder not connecting"))
                }          
            }
        }?;
        if let Some(ack) = ack {
            Ok(ack)
        } else {
            StateWaiter::wait(waiter.unwrap(), | | self.confirm_syn_ack()).await.ok_or_else(| | BuckyError::new(BuckyErrorCode::ErrorState, "builder not connecting"))
        }
    }

    fn to_weak(&self) -> WeakAcceptStreamBuilder {
        WeakAcceptStreamBuilder(Arc::downgrade(&self.0))
    }

    fn building_stream(&self) -> &StreamContainer {
        &self.0.stream
    } 

    fn package_stream(&self) -> Option<AcceptPackageStream> {
        match &*self.0.state.read().unwrap() {
            AcceptStreamState::Connecting(connecting) => connecting.package_stream.as_ref().map(|a| a.clone()), 
            _ => None
        }
    }
}

impl TryFrom<&WeakAcceptStreamBuilder> for AcceptStreamBuilder {
    type Error = BuckyError;
    fn try_from(w: &WeakAcceptStreamBuilder) -> BuckyResult<Self> {
        w.0.upgrade().ok_or_else(|| BuckyError::new(BuckyErrorCode::ErrorState, "builder released"))
            .map(|builder|  Self(builder))
    }
}

#[async_trait]
impl TunnelBuilder for AcceptStreamBuilder {
    fn sequence(&self) -> TempSeq {
        self.0.stream.sequence()
    }
    fn state(&self) -> TunnelBuilderState {
        match &*self.0.state.read().unwrap() {
            AcceptStreamState::Connecting(_) => TunnelBuilderState::Connecting, 
            AcceptStreamState::Establish => TunnelBuilderState::Establish, 
            AcceptStreamState::Closed => TunnelBuilderState::Closed, 
        }
    }

    async fn wait_establish(&self) -> Result<(), BuckyError> {
        let (state, waiter) = match &mut *self.0.state.write().unwrap() {
            AcceptStreamState::Connecting(connecting) => {
                (TunnelBuilderState::Connecting, Some(connecting.waiter.new_waiter()))
            }
            AcceptStreamState::Establish => (TunnelBuilderState::Establish, None), 
            AcceptStreamState::Closed => (TunnelBuilderState::Closed, None) 
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

impl OnPackage<TcpSynConnection, tcp::AcceptInterface> for AcceptStreamBuilder {
    fn on_package(&self, _pkg: &TcpSynConnection, interface: tcp::AcceptInterface) -> Result<OnPackageResult, BuckyError> {
        let builder = self.clone();
        task::spawn(async move {
            if let Ok(ack) = builder.wait_confirm().await.map(|s| s.tcp_syn_ack.clone()) {
                let _ = match interface.confirm_accept(vec![DynamicPackage::from(ack)]).await {
                    Ok(_) => builder.building_stream().as_ref().establish_with(
                        StreamProviderSelector::Tcp(
                            interface.socket().clone(), 
                            interface.key().clone(), 
                            None), 
                        builder.building_stream()).await, 
                    Err(err) => {
                        let _ = builder.building_stream().as_ref().cancel_connecting_with(&err);
                        Err(err)
                    }
                };
            }
        });
        Ok(OnPackageResult::Handled)
    }
}

impl OnPackage<SessionData> for AcceptStreamBuilder {
    fn on_package(&self, pkg: &SessionData, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        if pkg.is_syn() {
            // 第一次收到syn时，创建package stream action
            let package_stream = match self.package_stream() {
                Some(p) => Some(p),
                None => {
                    let stack = Stack::from(&self.0.stack);
                    let resend_interval = stack.config().tunnel.udp.holepunch_interval;
                    match &mut *self.0.state.write().unwrap() {
                        AcceptStreamState::Connecting(connecting) => {
                            match &connecting.package_stream {
                                Some(a) => Some(a.clone()), 
                                None => {
                                    let p = AcceptPackageStream::new(self.to_weak(), pkg.session_id, resend_interval);
                                    *&mut connecting.package_stream = Some(p.clone());
                                    Some(p)
                                }
                            }
                        },
                        _ => None
                    }
                }
            };
            if let Some(package_stream) = package_stream {
                package_stream.on_package(pkg, None)
            } else {
                Ok(OnPackageResult::Handled)
            }
        } else if pkg.is_syn_ack() {
            unreachable!()
        } else {
            if let Some(package_stream) = self.package_stream() {
                package_stream.on_package(pkg, None)
            } else {
                unreachable!()
            }
        }
    }
}

impl OnPackage<TcpSynConnection> for AcceptStreamBuilder {
    fn on_package(&self, pkg: &TcpSynConnection, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        info!("{} got reverse connect request {}", self, pkg);
        assert_eq!(pkg.reverse_endpoint.is_some(), true);
        let _ = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                AcceptStreamState::Connecting(connecting) => {
                    if connecting.reverse_tcp {
                        Err(BuckyError::new(BuckyErrorCode::ErrorState, "reverse connecting"))
                    } else {
                        connecting.reverse_tcp = true;
                        Ok(())
                    }
                }, 
                _ => Err(BuckyError::new(BuckyErrorCode::ErrorState, "not connecting"))
            }
        }.map_err(|e| {
            info!("{} ignore reverse connect request for {}", self, e);
            e
        })?;
        let builder = self.clone();
        let stack = Stack::from(&self.0.stack);
        // let net_listener = stack.net_manager().listener();
        let stream = builder.building_stream().clone();
        let key_stub = if let Some(key_stub) = stack.keystore().get_key_by_remote(stream.remote().0, true) {
            key_stub
        } else {
            stack.keystore().create_key(stream.as_ref().tunnel().remote_const(), false)
        };
        let remote_desc = pkg.from_device_desc.desc().clone();
        let remote_timestamp = pkg.from_device_desc.body().as_ref().unwrap().update_time();
        
        async fn reverse_connect(
            builder: AcceptStreamBuilder, 
            remote_desc: DeviceDesc, 
            remote_timestamp: Timestamp, 
            key: MixAesKey,
            /*, local: IpAddr*/
            remote_ep: Endpoint) -> Result<(), BuckyError> {
            let stream = builder.building_stream();
            let stack = builder.building_stream().as_ref().stack();
            let remote_id = stream.remote().0;
            let tunnel = stack.tunnel_manager().container_of(remote_id).unwrap();

            let interface = tcp::Interface::connect(
                // local, 
                remote_ep, 
                remote_id.clone(), 
                remote_desc, 
                key,
                stack.config().tunnel.tcp.connect_timeout).await?;
            
            let tcp_ack = builder.wait_confirm().await.map(|ack| {
                ack.tcp_syn_ack.clone()
            })?;
            
            let ep_pair = EndpointPair::from((Endpoint::default_of(&remote_ep), remote_ep));

            match interface.confirm_connect(
                &stack, 
                vec![DynamicPackage::from(tcp_ack)], 
                stack.config().tunnel.tcp.confirm_timeout).await {
                Ok(resp_box) => {
                    let resp_packages = resp_box.packages_no_exchange();
                    if resp_packages.len() != 1 || resp_packages[0].cmd_code() != PackageCmdCode::TcpAckAckConnection {
                        if let Some(tcp_tunnel) = tunnel.tunnel_of::<tunnel::tcp::Tunnel>(&ep_pair) {
                            tcp_tunnel.mark_dead(tcp_tunnel.state());
                        }
                        Err(BuckyError::new(BuckyErrorCode::InvalidData, "invalid response"))
                    } else {
                        let ack_ack: &TcpAckAckConnection = resp_packages[0].as_ref();

                        let tcp_tunnel: tunnel::tcp::Tunnel = tunnel.create_tunnel(ep_pair, ProxyType::None)?;
                        let _ = tcp_tunnel.pre_active(remote_timestamp);

                        match ack_ack.result {
                            TCP_ACK_CONNECTION_RESULT_OK => {
                                stream.as_ref().establish_with(
                                    StreamProviderSelector::Tcp(
                                        interface.socket().clone(), 
                                        interface.key().clone(), 
                                        None), 
                                    stream).await
                            }, 
                            TCP_ACK_CONNECTION_RESULT_REFUSED => {
                                // do nothing
                                Err(BuckyError::new(BuckyErrorCode::Reject, "remote rejected"))
                            }, 
                            _ => {
                                tcp_tunnel.mark_dead(tcp_tunnel.state());
                                Err(BuckyError::new(BuckyErrorCode::InvalidData, "invalid response"))
                            }
                        }
                    }
                } 
                Err(err) => {
                    if let Some(tcp_tunnel) = tunnel.tunnel_of::<tunnel::tcp::Tunnel>(&ep_pair) {
                        tcp_tunnel.mark_dead(tcp_tunnel.state());
                    }
                    Err(err)
                }
            }
        } 

        // for local in net_listener.ip_set() {
            for remote in pkg.reverse_endpoint.as_ref().unwrap() {
                info!("{} will reverse connect to {}", self, remote);
                // let local = *local;
                let remote = *remote;
                let builder = builder.clone();
                let remote_desc = remote_desc.clone();
                let key = key_stub.key.clone();
                task::spawn(async move {
                    let _ = reverse_connect(
                        builder.clone(), 
                        remote_desc,
                        remote_timestamp, 
                        key,
                        /*local, */
                        remote).await
                        .map_err(|e| {
                            info!("{} reverse connect to {} failed for {}", builder, remote, e);
                            e
                        });
                });
            }
        // }

        Ok(OnPackageResult::Handled)
    }
}


impl PingClientCalledEvent<PackageBox> for AcceptStreamBuilder {
    fn on_called(&self, called: &SnCalled, caller_box: PackageBox) -> Result<(), BuckyError> {
        let builder = self.clone();
        let active_pn_list = called.active_pn_list.clone();
        task::spawn(async move {
            let _ = builder.build(caller_box, active_pn_list).await;
        });
        Ok(())
    }
}

impl OnPackage<AckProxy, &DeviceId> for AcceptStreamBuilder {
    fn on_package(&self, pkg: &AckProxy, proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> {
        if let Some(proxy_builder) = match &*self.0.state.read().unwrap() {
            AcceptStreamState::Connecting(connecting) => connecting.proxy.clone(),
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
