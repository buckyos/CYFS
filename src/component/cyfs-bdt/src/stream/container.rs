mod dep {
    pub use super::super::{
        package::PackageStream, stream_provider::StreamProvider, tcp::TcpStream,
    };
    pub use crate::{
        types::*, 
        interface::*,
        protocol::{*, v0::*},
        stack::{Stack, WeakStack},
        tunnel::{
            self, AcceptReverseTcpStream, AcceptStreamBuilder, BuildTunnelAction,
            BuildTunnelParams, ConnectPackageStream, ConnectStreamAction, ConnectStreamBuilder,
            ConnectStreamState, ConnectTcpStream, ProxyType, StreamConnectorSelector, Tunnel,
            TunnelBuilder, TunnelContainer, TunnelGuard, TunnelState,
        },
        types::*,
    };
    pub use async_std::{future, sync::Arc, task};
    pub use cyfs_base::*;
    pub use futures::future::{AbortHandle, Abortable, Aborted};
    pub use std::{fmt, future::Future, net::Shutdown, ops::Deref, sync::RwLock, time::Duration};
}

const ANSWER_MAX_LEN: usize = 1024*25;

mod connector {
    use super::dep::*;
    use super::StreamContainer;

    pub struct Connector {
        pub question: Vec<u8>,
        pub waiter: StateWaiter,
        pub state: State,
        pub start_at: Timestamp,
    }

    pub enum State {
        Unknown,
        Tcp(TcpProvider),
        Reverse(ReverseProvider),
        Package(PackageProvider),
        Builder(BuilderProvider),
    }

    impl State {
        pub fn remote_timestamp(&self) -> Option<Timestamp> {
            match self {
                Self::Unknown => None,
                Self::Tcp(tcp) => Some(tcp.1),
                Self::Reverse(tcp) => Some(tcp.remote_timestamp),
                Self::Package(package) => Some(package.1),
                Self::Builder(_) => None,
            }
        }
        pub fn from(
            params: (&StreamContainer, StreamConnectorSelector),
        ) -> (Self, Box<dyn Provider>) {
            match params.1 {
                StreamConnectorSelector::Package(remote_timestamp) => {
                    let provider = PackageProvider::new(params.0, remote_timestamp);
                    (State::Package(provider.clone()), Box::new(provider))
                }
                StreamConnectorSelector::Tcp(tunnel, remote_timestamp) => {
                    if !tunnel.is_reverse() {
                        let provider = TcpProvider(tunnel, remote_timestamp);
                        (State::Tcp(provider.clone()), Box::new(provider))
                    } else {
                        let provider = ReverseProvider::new(params.0, tunnel, remote_timestamp);
                        (State::Reverse(provider.clone()), Box::new(provider))
                    }
                }
                StreamConnectorSelector::Builder(builder) => {
                    let provider = BuilderProvider(builder);
                    (State::Builder(provider.clone()), Box::new(provider))
                }
            }
        }
    }

    pub trait Provider {
        fn begin_connect(&self, stream: &StreamContainer);
    }

    #[derive(Clone)]
    pub struct TcpProvider(tunnel::tcp::Tunnel, Timestamp);

    impl fmt::Display for TcpProvider {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "TcpProvider:{{tunnel:{}, remote_timestamp:{}}}",
                self.0, self.1
            )
        }
    }

    impl Provider for TcpProvider {
        fn begin_connect(&self, stream: &StreamContainer) {
            debug!("{} connect with {}", stream, self);
            let action = ConnectTcpStream::new(
                stream.0.stack.clone(),
                stream.clone(),
                self.0.clone(),
            );
            let stream = stream.clone();
            task::spawn(async move {
                match action.wait_pre_establish().await {
                    ConnectStreamState::PreEstablish => match action.continue_connect().await {
                        Ok(selector) => {
                            let _ = stream.establish_with(selector).await;
                        }
                        Err(err) => {
                            let _ = stream.cancel_connecting_with(&err);
                        }
                    },
                    _ => {
                        let _ = stream.cancel_connecting_with(&BuckyError::new(
                            BuckyErrorCode::ErrorState,
                            "action not pre establish",
                        ));
                    }
                }
            });
        }
    }

    #[derive(Clone)]
    pub struct PackageProvider(ConnectPackageStream, Timestamp);

    impl PackageProvider {
        fn new(stream: &StreamContainer, remote_timestamp: Timestamp) -> Self {
            Self(ConnectPackageStream::new(stream.clone()), remote_timestamp)
        }
    }

    impl fmt::Display for PackageProvider {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "PackageProvider:{{remote_timestamp:{}}}", self.1)
        }
    }

    impl Provider for PackageProvider {
        fn begin_connect(&self, stream: &StreamContainer) {
            debug!("{} connect with {}", stream, self);
            let action = self.0.clone();
            let stream = stream.clone();
            task::spawn(async move {
                match action.wait_pre_establish().await {
                    ConnectStreamState::PreEstablish => match action.continue_connect().await {
                        Ok(selector) => {
                            let _ = stream.establish_with(selector).await;
                        }
                        Err(err) => {
                            let _ = stream.cancel_connecting_with(&err);
                        }
                    },
                    _ => {
                        let _ = stream.cancel_connecting_with(&BuckyError::new(
                            BuckyErrorCode::ErrorState,
                            "action not pre establish",
                        ));
                    }
                }
            });

            self.0.begin();
        }
    }

    impl AsRef<ConnectPackageStream> for PackageProvider {
        fn as_ref(&self) -> &ConnectPackageStream {
            &self.0
        }
    }

    #[derive(Clone)]
    pub struct ReverseProvider {
        pub remote_timestamp: Timestamp,
        pub local: Endpoint,
        pub action: AcceptReverseTcpStream,
    }

    impl fmt::Display for ReverseProvider {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "ReverseProvider:{{local：{}, remote_timestamp:{}}}",
                self.local, self.remote_timestamp
            )
        }
    }

    impl ReverseProvider {
        fn new(
            stream: &StreamContainer,
            tunnel: tunnel::tcp::Tunnel,
            remote_timestamp: Timestamp,
        ) -> Self {
            Self {
                remote_timestamp,
                local: *tunnel.local(),
                action: AcceptReverseTcpStream::new(
                    stream.clone(),
                    *tunnel.local(),
                    *tunnel.remote(),
                ),
            }
        }
    }

    impl Provider for ReverseProvider {
        fn begin_connect(&self, stream: &StreamContainer) {
            debug!("{} connect with {}", stream, self);
            let provider = self.clone();
            let stream = stream.clone();
            let mut syn_tcp = stream.syn_tcp_stream().unwrap();

            let stack = stream.stack();
            let listener = stack.net_manager().listener();
            let mut endpoints = vec![];
            for t in listener.tcp() {
                let outer = t.outer();
                if outer.is_some() {
                    let outer = outer.unwrap();
                    if outer == self.local || t.local() == self.local {
                        endpoints.push(outer);
                    }
                } else {
                    endpoints.push(self.local);
                }
            }
            syn_tcp.reverse_endpoint = Some(endpoints);

            task::spawn(async move {
                loop {
                    match future::timeout(
                        stream.stack().config().stream.stream.package.connect_resend_interval,
                        provider.action.wait_pre_establish(),
                    )
                    .await
                    {
                        Err(_) => {
                            if let Some(tunnel) = stream.tunnel() {
                                let _ = tunnel.send_packages(vec![DynamicPackage::from(syn_tcp.clone())]);
                            }
                        }
                        Ok(state) => {
                            match state {
                                ConnectStreamState::PreEstablish => {
                                    match provider.action.continue_connect().await {
                                        Ok(selector) => {
                                            let _ = stream.establish_with(selector).await;
                                        }
                                        Err(err) => {
                                            let _ = stream.cancel_connecting_with(&err);
                                        }
                                    }
                                }
                                _ => {
                                    let _ =
                                        stream.cancel_connecting_with(&BuckyError::new(
                                            BuckyErrorCode::ErrorState,
                                            "action not pre establish",
                                        ));
                                }
                            };
                            break;
                        }
                    }
                }
            });
        }
    }

    #[derive(Clone)]
    pub struct BuilderProvider(ConnectStreamBuilder);

    impl Provider for BuilderProvider {
        fn begin_connect(&self, _stream: &StreamContainer) {
            let builder = self.0.clone();
            task::spawn(async move {
                builder.build().await;
            });
        }
    }

    impl AsRef<ConnectStreamBuilder> for BuilderProvider {
        fn as_ref(&self) -> &ConnectStreamBuilder {
            &self.0
        }
    }
}

mod acceptor {
    use super::dep::*;

    pub struct Acceptor {
        pub remote_id: IncreaseId,
        pub waiter: StateWaiter,
        pub builder: AcceptStreamBuilder,
    }
}

use async_std::io::prelude::{Read, Write};
pub use dep::Shutdown;
use dep::*;
use futures::io::ErrorKind;
use log::*;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

enum StreamConnectingState {
    Connect(connector::Connector),
    Accept(acceptor::Acceptor),
}

struct StreamEstablishState {
    start_at: Timestamp,
    remote_timestamp: Timestamp,
    provider: Box<dyn StreamProvider>,
}

#[derive(Clone)]
pub struct Config {
    pub retry_sn_timeout: Duration, 
    pub connect_timeout: Duration,
    pub nagle: Duration,
    pub recv_timeout: Duration,
    pub recv_buffer: usize,
    pub send_buffer: usize,
    pub drain: f32,
    pub tcp: super::tcp::Config,
    pub package: super::package::Config,
}

impl Config {
    pub fn recv_drain(&self) -> usize {
        (self.recv_buffer as f32 * self.drain) as usize
    }

    pub fn send_drain(&self) -> usize {
        (self.send_buffer as f32 * self.drain) as usize
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum StreamState {
    Connecting,
    Establish(Timestamp),
    Closing,
    Closed,
}
impl fmt::Display for StreamState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamState::Connecting => write!(f, "StreamState::Connecting"),
            StreamState::Establish(remote_timestamp) => {
                write!(f, "StreamState::Establish({})", remote_timestamp)
            }
            StreamState::Closing => write!(f, "StreamState::Closing"),
            StreamState::Closed => write!(f, "StreamState::Closed"),
        }
    }
}

enum StreamStateImpl {
    Initial(TunnelGuard),
    Connecting(StreamConnectingState, TunnelGuard),
    Establish(StreamEstablishState, TunnelGuard),
    Closing(StreamEstablishState, TunnelGuard),
    Closed,
}

struct PackageStreamProviderSelector {}
pub enum StreamProviderSelector {
    Package(IncreaseId /*remote id*/, Option<SessionData>),
    Tcp(async_std::net::TcpStream, MixAesKey, Option<TcpAckConnection>),
}

struct StreamContainerImpl {
    stack: WeakStack, 
    remote_device: DeviceId, 
    remote_port: u16,
    local_id: IncreaseId,
    sequence: TempSeq,
    state: RwLock<StreamStateImpl>,
    answer_data: RwLock<Option<Vec<u8>>>,
}

#[derive(Clone)]
pub struct StreamContainer(Arc<StreamContainerImpl>);


impl fmt::Display for StreamContainer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StreamContainer {{sequence:{:?}, local:{}, remote:{}, port:{}, id:{} }}",
            self.sequence(),
            self.stack().local_device_id(),
            self.remote().0,
            self.remote().1,
            self.local_id()
        )
    }
}

impl StreamContainer {
    pub(super) fn new(
        weak_stack: WeakStack,
        tunnel: TunnelGuard,
        remote_port: u16,
        local_id: IncreaseId,
        sequence: TempSeq,
    ) -> StreamContainer {
        let stream_impl = StreamContainerImpl {
            stack: weak_stack, 
            remote_device: tunnel.remote().clone(), 
            remote_port,
            local_id,
            sequence: sequence,
            state: RwLock::new(StreamStateImpl::Initial(tunnel)),
            answer_data: RwLock::new(None)
        };

        StreamContainer(Arc::new(stream_impl))
    }

    //收到ack以后继续连接, 可以完成时在builder里面调用 establish_with
    pub(super) fn accept(&self, remote_id: IncreaseId) {
        let state = &mut *self.0.state.write().unwrap();
        if let StreamStateImpl::Initial(tunnel) = &*state {
            info!("{} initial=>accepting remote_id {}", self, remote_id);
            *state =
                StreamStateImpl::Connecting(StreamConnectingState::Accept(acceptor::Acceptor {
                    remote_id,
                    waiter: StateWaiter::new(),
                    builder: AcceptStreamBuilder::new(self.0.stack.clone(), self.clone(), tunnel.as_ref().clone()),
                }), tunnel.clone());

            let stream = self.clone();
            task::spawn(async move {
                match future::timeout(
                    stream
                        .stack()
                        .config()
                        .stream
                        .stream
                        .connect_timeout,
                    stream.wait_establish(),
                )
                .await
                {
                    Ok(r) => r,
                    Err(err) => {
                        let err = err.into();
                        match stream.cancel_connecting_with(&err) {
                            Ok(_) => Err(err),
                            Err(_) => Ok(()),
                        }
                    }
                }
            });
        } else {
            unreachable!("not initial")
        }
    }

    // 开始发起连接，连接完成或者失败时返回
    pub(super) async fn connect(
        &self,
        question: Vec<u8>,
        build_params: BuildTunnelParams,
    ) -> BuckyResult<()> {
        // initial connector
        let connector = connector::Connector {
            question: Vec::from(question),
            waiter: StateWaiter::new(),
            state: connector::State::Unknown,
            start_at: bucky_time_now(),
        };

        // enter connecting connector unknown
        let tunnel = {
            let mut state = self.0.state.write().unwrap();
            if let StreamStateImpl::Initial(tunnel) = &*state {
                info!("{} initial=>connecting", self);
                let tunnel = tunnel.clone();
                *state = StreamStateImpl::Connecting(StreamConnectingState::Connect(connector), tunnel.clone());
                tunnel
            } else {
                unreachable!("not initial")
            }
        };

        // 从tunnel container返回要用的connector provider
        let _ = match tunnel.select_stream_connector(build_params, self.clone()).await
        {
            Ok(selector) => {
                let (connector_state, connector_provider) =
                    connector::State::from((self, selector));
                // enter connecting connector provider
                {
                    let state = &mut *self.0.state.write().unwrap();
                    if let StreamStateImpl::Connecting(ref mut connecting, _) = state {
                        if let StreamConnectingState::Connect(ref mut connector) = connecting {
                            if let connector::State::Unknown = connector.state {
                                connector.state = connector_state;
                                connector.start_at = bucky_time_now();
                            } else {
                                unreachable!()
                            }
                        } else {
                            unreachable!()
                        }
                    } else {
                        unreachable!()
                    }
                }
                connector_provider.begin_connect(self);
                Ok(())
            }
            Err(err) => Err(err),
        }?;

        match future::timeout(
            self.stack().config().stream.stream.connect_timeout,
            self.wait_establish(),
        )
        .await
        {
            Ok(r) => r,
            Err(err) => {
                let err = err.into();
                match self.cancel_connecting_with(&err) {
                    Ok(_) => Err(err),
                    Err(_) => Ok(()),
                }
            }
        }
    }

    //以selector指定的方式联通
    pub(crate) async fn establish_with(
        &self,
        selector: StreamProviderSelector
    ) -> BuckyResult<()> {
        let tunnel = self.tunnel()
            .ok_or_else(|| BuckyError::new(
                BuckyErrorCode::ErrorState,
                "tunnel not active",
            )).map_err(|e| {
                error!("{} try establish failed for {}", self, e);
                e
            })?;
        let remote_timestamp = match tunnel.wait_active().await {
            TunnelState::Active(remote_timetamp) => Ok(remote_timetamp),
            _ => Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "tunnel not active",
            )),
        }
        .map_err(|e| {
            error!("{} try establish failed for {}", self, e);
            e
        })?;

        let (provider, provider_stub, answer_data) = match selector {
            StreamProviderSelector::Package(remote_id, ack) => {
                let answer_data = match ack {
                    Some(session_data) => {
                        if session_data.payload.as_ref().len() > 0 {
                            let mut answer = vec![0; session_data.payload.as_ref().len()];
                            answer.copy_from_slice(session_data.payload.as_ref());
                            answer
                        } else {
                            vec![]
                        }
                    },
                    _ => vec![],
                };

                let stream = PackageStream::new(self, tunnel.as_ref(), self.local_id().clone(), remote_id)?;
                (
                    Box::new(stream.clone()) as Box<dyn StreamProvider>,
                    Box::new(stream) as Box<dyn StreamProvider>,
                    answer_data,
                )
            }
            StreamProviderSelector::Tcp(socket, key, ack) => {
                let answer_data = match ack {
                    Some(tcp_ack_connection) => {
                        if tcp_ack_connection.payload.as_ref().len() > 0 {
                            let mut answer = vec![0; tcp_ack_connection.payload.as_ref().len()];
                            answer.copy_from_slice(tcp_ack_connection.payload.as_ref());
                            answer
                        } else {
                            vec![]
                        }
                    },
                    _ => vec![],
                };

                let stream = TcpStream::new(self.clone(), socket, key.enc_key)?;
                (
                    Box::new(stream.clone()) as Box<dyn StreamProvider>,
                    Box::new(stream) as Box<dyn StreamProvider>,
                    answer_data,
                )
            }
        };

        let state = &mut *self.0.state.write().unwrap();
        let waiter = match state {
            StreamStateImpl::Connecting(ref mut connecting_state, tunnel) => match connecting_state {
                StreamConnectingState::Accept(ref mut acceptor) => {
                    let waiter = acceptor.waiter.transfer();
                    info!("{} accepting=>establish with provider {}", self, provider);
                    *state = StreamStateImpl::Establish(StreamEstablishState {
                        start_at: bucky_time_now(),
                        remote_timestamp,
                        provider: provider_stub,
                    }, tunnel.clone());
                    Ok(waiter)
                }
                StreamConnectingState::Connect(ref mut connector) => {
                    let waiter = connector.waiter.transfer();
                    info!("{} connecting=>establish with {}", self, provider);
                    *state = StreamStateImpl::Establish(StreamEstablishState {
                        start_at: bucky_time_now(),
                        remote_timestamp,
                        provider: provider_stub,
                    }, tunnel.clone());

                    if answer_data.len() > 0 {
                        let data = &mut *self.0.answer_data.write().unwrap();
                        *data = Some(answer_data);
                    }

                    Ok(waiter)
                }
            },
            _ => Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "stream not connecting",
            )),
        }
        .map_err(|e| {
            error!("{} try establish failed for {}", self, e);
            e
        })?;
        // 开始stream provider的收发
        provider.start(self);
        // 唤醒等待 establish 的waiter
        waiter.wake();
        Ok(())
    }

    pub(crate) fn cancel_connecting_with(&self, err: &BuckyError) -> BuckyResult<()> {
        warn!("{} cancel connecting with error: {}", self, err);
        let state = &mut *self.0.state.write().unwrap();
        let (waiter, state_dump) = match state {
            StreamStateImpl::Connecting(ref mut connecting_state, tunnel) => match connecting_state {
                StreamConnectingState::Accept(ref mut acceptor) => {
                    let waiter = acceptor.waiter.transfer();
                    info!("{} accepting=>closed", self);
                    *state = StreamStateImpl::Closed;
                    Ok((waiter, None))
                }
                StreamConnectingState::Connect(ref mut connector) => {
                    let waiter = connector.waiter.transfer();
                    let tunnel = tunnel.clone();
                    info!("{} connecting=>closed", self);
                    let state_dump = connector
                        .state
                        .remote_timestamp()
                        .map(|r| (tunnel, r, connector.start_at));
                    *state = StreamStateImpl::Closed;
                    Ok((waiter, state_dump))
                }
            },
            _ => Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "stream not connecting",
            )),
        }?;
        if let Some((tunnel, remote_timestamp, start_at)) = state_dump {
            error!("{} mark tunnel dead", self);
            let _ = tunnel.mark_dead(remote_timestamp, start_at);
        }
        // 唤醒等待 establish 的waiter
        waiter.wake();
        Ok(())
    }

    pub(crate) async fn wait_establish(&self) -> BuckyResult<()> {
        let waiter = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StreamStateImpl::Connecting(ref mut connecting, _) => {
                    let waiter = match connecting {
                        StreamConnectingState::Accept(ref mut acceptor) => {
                            acceptor.waiter.new_waiter()
                        }
                        StreamConnectingState::Connect(ref mut connector) => {
                            connector.waiter.new_waiter()
                        }
                    };
                    Ok(Some(waiter))
                }
                StreamStateImpl::Establish(..) => Ok(None),
                _ => {
                    warn!("{} wait establish failed, neither StreamStateImpl::Connecting nor StreamStateImpl::Establish", self);
                    Err(BuckyError::new(
                        BuckyErrorCode::ErrorState,
                        "stream not established",
                    ))
                }
            }
        }?;

        if let Some(waiter) = waiter {
            match StateWaiter::wait(waiter, || self.state()).await {
                StreamState::Establish(_) => Ok(()),
                _ => {
                    error!(
                        "{} wait establish failed, for stream state not establish,state={}",
                        self,
                        self.state()
                    );
                    Err(BuckyError::new(
                        BuckyErrorCode::ErrorState,
                        "stream not established",
                    ))
                }
            }
        } else {
            Ok(())
        }
    }

    pub(crate) fn syn_session_data(&self) -> Option<SessionData> {
        {
            match &*self.0.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting, _) => match connecting {
                    StreamConnectingState::Connect(connector) => Some(connector.question.clone()),
                    _ => {
                        unreachable!()
                    }
                },
                _ => None,
            }
        }
        .map(|question| {
            let mut session = SessionData::new();
            session.stream_pos = 0;
            session.syn_info = Some(SessionSynInfo {
                sequence: self.sequence(),
                from_session_id: self.local_id().clone(),
                to_vport: self.remote().1,
            });
            session.session_id = self.local_id().clone();
            session.send_time = bucky_time_now();
            session.flags_add(SESSIONDATA_FLAG_SYN);
            session.payload = TailedOwnedData::from(question);
            session
        })
    }

    pub(crate) fn syn_ack_session_data(&self, answer: &[u8]) -> Option<SessionData> {
        {
            match &*self.0.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting, _) => match connecting {
                    StreamConnectingState::Accept(acceptor) => Some(acceptor.remote_id.clone()),
                    _ => {
                        unreachable!()
                    }
                },
                _ => None,
            }
        }
        .map(|remote_id| {
            let mut session = SessionData::new();
            session.stream_pos = 0;
            session.syn_info = Some(SessionSynInfo {
                sequence: self.sequence(),
                from_session_id: self.local_id().clone(),
                to_vport: 0,
            });
            session.ack_stream_pos = 0;
            session.send_time = bucky_time_now();
            session.flags_add(SESSIONDATA_FLAG_SYN | SESSIONDATA_FLAG_ACK);
            session.to_session_id = Some(remote_id.clone());
            session.session_id = remote_id;
            let mut payload = vec![0u8; answer.len()];
            payload.copy_from_slice(answer);
            session.payload = TailedOwnedData::from(payload);
            session
        })
    }

    pub(crate) fn syn_tcp_stream(&self) -> Option<TcpSynConnection> {
        {
            match &*self.0.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting, _) => match connecting {
                    StreamConnectingState::Connect(connector) => Some(connector.question.clone()),
                    _ => {
                        unreachable!()
                    }
                },
                _ => None,
            }
        }
        .map(|question| {
            let local_device = self.stack().sn_client().ping().default_local();
            TcpSynConnection {
                sequence: self.sequence(),
                result: 0u8,
                to_vport: self.remote().1,
                from_session_id: self.local_id(),
                from_device_desc: local_device,
                to_device_id: self.remote().0.clone(),
                reverse_endpoint: None,
                payload: TailedOwnedData::from(question),
            }
        })
    }

    pub(crate) fn ack_tcp_stream(&self, answer: &[u8]) -> Option<TcpAckConnection> {
        {
            match &*self.0.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting, _) => match connecting {
                    StreamConnectingState::Accept(acceptor) => Some(acceptor.remote_id.clone()),
                    _ => {
                        unreachable!()
                    }
                },
                _ => None,
            }
        }
        .map(|remote_id| {
            let mut payload = vec![0u8; answer.len()];
            payload.copy_from_slice(answer);

            TcpAckConnection {
                sequence: self.sequence(),
                to_session_id: remote_id,
                result: TCP_ACK_CONNECTION_RESULT_OK,
                to_device_desc: self.stack().sn_client().ping().default_local(),
                payload: TailedOwnedData::from(payload),
            }
        })
    }

    pub(crate) fn ack_ack_tcp_stream(&self, result: u8) -> TcpAckAckConnection {
        TcpAckAckConnection {
            sequence: self.sequence(),
            result,
        }
    }

    pub fn tunnel(&self) -> Option<TunnelGuard> {
        match &*self.0.state.read().unwrap() {
            StreamStateImpl::Initial(tunnel) => Some(tunnel.clone()),
            StreamStateImpl::Connecting(_, tunnel) => Some(tunnel.clone()),
            StreamStateImpl::Establish(_, tunnel) => Some(tunnel.clone()),
            StreamStateImpl::Closing(_, tunnel) => Some(tunnel.clone()),
            StreamStateImpl::Closed => None
        }
    }

    pub fn remote_id(&self) -> IncreaseId {
        match &*self.0.state.read().unwrap() {
            StreamStateImpl::Establish(est, _) => {
                est.provider.remote_id()
            }
            StreamStateImpl::Closing(est, _) => {
                est.provider.remote_id()
            },
            _ => IncreaseId::default(),
        }
    }

    pub fn state(&self) -> StreamState {
        match &*self.0.state.read().unwrap() {
            StreamStateImpl::Initial(_) => unreachable!(),
            StreamStateImpl::Connecting(..) => StreamState::Connecting,
            StreamStateImpl::Establish(establish, ..) => {
                StreamState::Establish(establish.remote_timestamp)
            }
            StreamStateImpl::Closing(..) => StreamState::Closing,
            StreamStateImpl::Closed => StreamState::Closed,
        }
    }

    pub(crate) fn is_connecting(&self) -> bool {
        let state = self.0.state.read().unwrap();
        let s1 = state.deref();
        match s1 {
            StreamStateImpl::Connecting(..) => true,
            _ => false,
        }
    }

    pub(crate) fn acceptor(&self) -> Option<AcceptStreamBuilder> {
        if let StreamStateImpl::Connecting(connecting, _) = &*self.0.state.read().unwrap() {
            if let StreamConnectingState::Accept(acceptor) = connecting {
                return Some(acceptor.builder.clone());
            }
        }
        None
    }

    pub(crate) fn stack(&self) -> Stack {
        Stack::from(&self.0.stack)
    }

    pub(crate) fn break_with_error(&self, err: BuckyError) {
        error!("{} break with err {}", self, err);
        let state_dump = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                StreamStateImpl::Establish(establish, tunnel) => {
                    let tunnel = tunnel.clone();
                    let state_dump = Some((tunnel, establish.remote_timestamp, establish.start_at));
                    *state = StreamStateImpl::Closed;
                    state_dump
                }
                _ => None,
            }
        };
        if let Some((tunnel, remote_timestamp, start_at)) = state_dump {
            debug!("{} mark tunnel dead for break", self);
            let _ = tunnel.mark_dead(remote_timestamp, start_at);
        }
        self.stack().stream_manager().remove_stream(self);
    }

    pub(super) fn on_shutdown(&self) {
        *self.0.state.write().unwrap() = StreamStateImpl::Closed;
        self.stack().stream_manager().remove_stream(self);
    }

    pub async fn confirm(&self, answer: &[u8]) -> BuckyResult<()> {
        if answer.len() > ANSWER_MAX_LEN {
            return Err(BuckyError::new(
                BuckyErrorCode::Failed,
                format!("answer's length large than {}", ANSWER_MAX_LEN),
            ));
        }

        let builder = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Connecting(connecting, _) => match connecting {
                    StreamConnectingState::Accept(acceptor) => Ok(acceptor.builder.clone()),
                    _ => Err(BuckyError::new(
                        BuckyErrorCode::ErrorState,
                        "confirm on error state",
                    )),
                },
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "confirm on error state",
                )),
            }?
        };
        match builder.confirm(answer) {
            Err(e) => {
                error!("{} confirm failed for {}", self, e);
                Err(e)
            }
            Ok(v) => {
                info!("{} confirmed", self);
                Ok(v)
            }
        }
    }

    pub fn shutdown(&self, which: Shutdown) -> std::io::Result<()> {
        info!("{} shutdown", self);
        let provider = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Establish(s, _) => Ok(s.provider.clone_as_provider()),
				StreamStateImpl::Closed => Ok(None),
                _ => {
                    //TODO 其他状态暂时不支持shutdown
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "not support when state is not establish",
                    ))
                }
            }
        }
        .map_err(|e| {
            error!("{} shutdown failed for {}", self, e);
            e
        })?;

        if let Some(provider) = provider {
            provider.shutdown(which, &self)
        } else {
            Ok(())
        }
    }

    pub fn readable(&self) -> StreamReadableFuture {
        StreamReadableFuture {
            stream: self.clone(),
        }
    }

    fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<usize>> {
        let provider = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Establish(s, _) => Some(s.provider.clone_as_provider()),
                _ => None,
            }
        };
        if let Some(provider) = provider {
            provider.poll_readable(cx)
        } else {
            Poll::Ready(Err(std::io::Error::new(
                ErrorKind::NotConnected,
                "not establish",
            )))
        }
    }

    fn has_first_answer(&self) -> bool {
        self.0.answer_data.read().unwrap().is_some()
    }

    fn read_first_answer(&self, buf: &mut [u8]) -> usize {
        if self.has_first_answer() {
            let answer_data = &mut *self.0.answer_data.write().unwrap();
            match answer_data {
                Some(answer_buf) => {
                    let mut read_len = answer_buf.len();
                    if read_len > buf.len() {
                        read_len = buf.len();
                    }
        
                    buf[..read_len].copy_from_slice(&answer_buf[..read_len]);
        
                    if read_len == answer_buf.len() {
                        *answer_data = None;
                    } else {
                        let data = &answer_buf[read_len..];

                        *answer_data = Some(data.to_vec())
                    }
        
                    read_len
                },
                None => 0
            }
        } else {
            0
        }
    }

    fn poll_read(&self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        debug!("{} poll read {} bytes", self, buf.len());
        let read_len = self.read_first_answer(buf);
        if read_len > 0 {
            return Poll::Ready(Ok(read_len));
        }

        let provider = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Initial(_) | StreamStateImpl::Connecting(..) => {
                    trace!(
                        "{} poll-write in initial/connecting.",
                        self,
                    );
                    None
                }, 
                StreamStateImpl::Establish(s, _) => Some(s.provider.clone_as_provider()),
                StreamStateImpl::Closing(s, _) => Some(s.provider.clone_as_provider()), 
                _ => {
                    return Poll::Ready(Ok(0));
                }
            }
        };
        
        if let Some (provider) = provider {
            let read_len = self.read_first_answer(buf);
            if read_len > 0 {
                return Poll::Ready(Ok(read_len));
            }
            provider.poll_read(cx, buf)
        } else {
            let waker = cx.waker().clone();
            let stream = self.clone();
            task::spawn(async move {
                let _ = stream.wait_establish().await;
                waker.wake();
            });
            Poll::Pending
        }
    }

    fn poll_write(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        debug!("{} poll write {} bytes", self, buf.len());
        self.poll_write_wait_establish(cx.waker().clone(), |provider| {
            provider.poll_write(cx, buf)
        })
    }

    fn poll_flush(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        debug!("{} poll flush", self);
        self.poll_write_wait_establish(cx.waker().clone(), |provider| {
            provider.poll_flush(cx)
        })
    }

    fn poll_close(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        debug!("{} poll close", self);
        let provider = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Establish(s, _) => Some(s.provider.clone_as_provider()),
                StreamStateImpl::Initial(_) => {
                    debug!("poll-close, {} in initial.", self);
                    None
                }
                StreamStateImpl::Connecting(..) => {
                    debug!("poll-close, {} in connecting.", self);
                    None
                }
                StreamStateImpl::Closing(s, _) => {
                    debug!("poll-close, {} in closing.", self);
                    Some(s.provider.clone_as_provider())
                }
                StreamStateImpl::Closed => {
                    debug!("poll-close, {} in closed ready.", self);
                    return Poll::Ready(Ok(()));
                }
            }
        };

        match provider {
            Some(provider) => provider.poll_close(cx),
            None => {
                let _ = self.cancel_connecting_with(&BuckyError::new(
                    BuckyErrorCode::ConnectionAborted,
                    "user close",
                ));
                Poll::Ready(Err(std::io::Error::new(
                    ErrorKind::ConnectionAborted,
                    "close by user",
                )))
            }
        }
    }

    pub fn remote(&self) -> (&DeviceId, u16) {
        (&self.0.remote_device, self.0.remote_port)
    }

    pub fn sequence(&self) -> TempSeq {
        self.0.sequence
    }

    pub fn local_id(&self) -> IncreaseId {
        self.0.local_id
    }

    pub fn local_ep(&self) -> Option<Endpoint> {
        let state = &*self.0.state.read().unwrap();
        match state {
            StreamStateImpl::Establish(s, _) => Some(*s.provider.local_ep()),
            _ => None,
        }
    }

    pub fn remote_ep(&self) -> Option<Endpoint> {
        let state = &*self.0.state.read().unwrap();
        match state {
            StreamStateImpl::Establish(s, _) => Some(*s.provider.remote_ep()),
            _ => None,
        }
    }

    fn poll_write_wait_establish<R>(
        &self,
        waker: Waker,
        mut proc: impl FnMut(&dyn StreamProvider) -> Poll<std::io::Result<R>>,
    ) -> Poll<std::io::Result<R>> {
        let provider = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Establish(s, _) => Some(s.provider.clone_as_provider()),
                StreamStateImpl::Initial(..) | StreamStateImpl::Connecting(..) => {
                    trace!(
                        "{} poll-write in initial/connecting.",
                        self,
                    );
                    None
                }, 
                _ => {
                    let msg = format!("{} poll-write in close.", self);
                    error!("{}", msg);
                    return Poll::Ready(Err(std::io::Error::new(ErrorKind::NotConnected, msg)));
                }
            }
        };

        match provider {
            Some(provider) => proc(&*provider),
            None => {
                let stream = self.clone();
                task::spawn(async move {
                    let _ = stream.wait_establish().await;
                    waker.wake();
                });
                Poll::Pending
            }
        }
    }
}

pub struct StreamReadableFuture {
    stream: StreamContainer,
}

impl Future for StreamReadableFuture {
    type Output = std::io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.stream.poll_readable(cx)
    }
}


impl OnPackage<TcpSynConnection, tcp::AcceptInterface> for StreamContainer {
    fn on_package(
        &self,
        pkg: &TcpSynConnection,
        interface: tcp::AcceptInterface,
    ) -> BuckyResult<OnPackageResult> {
        debug!("{} on package {} from {}", self, pkg, interface);
        // syn tcp 直接转给builder
        let state = &*self.0.state.read().unwrap();
        let builder = match state {
            StreamStateImpl::Connecting(connecting, _) => match connecting {
                StreamConnectingState::Accept(acceptor) => Ok(acceptor.builder.clone()),
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "syn session data on error state",
                )),
            },
            _ => Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "syn session data on error state",
            )),
        }?;
        builder.on_package(pkg, interface)
    }
}

impl OnPackage<SessionData> for StreamContainer {
    fn on_package(&self, pkg: &SessionData, _: Option<()>) -> BuckyResult<OnPackageResult> {
        if pkg.is_syn() {
            debug!("{} on package {}", self, pkg);
            // syn session data直接转给builder
            let state = &*self.0.state.read().unwrap();
            let builder = match state {
                StreamStateImpl::Connecting(connecting, _) => match connecting {
                    StreamConnectingState::Accept(acceptor) => Ok(acceptor.builder.clone()),
                    _ => Err(BuckyError::new(
                        BuckyErrorCode::ErrorState,
                        "syn session data on error state",
                    )),
                },
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "syn session data on error state",
                )),
            }?;
            builder.on_package(pkg, None)
        } else if pkg.is_syn_ack() {
            debug!("{} on package {}", self, pkg);
            // 传给 connector provider
            let handler: Box<dyn OnPackage<SessionData>> = match &*self.0.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting, _) => match connecting {
                    StreamConnectingState::Connect(connector) => match &connector.state {
                        connector::State::Package(package_provider) => {
                            Ok(Box::new(package_provider.as_ref().clone())
                                as Box<dyn OnPackage<SessionData>>)
                        }
                        connector::State::Builder(builder_provider) => {
                            Ok(Box::new(builder_provider.as_ref().clone())
                                as Box<dyn OnPackage<SessionData>>)
                        }
                        _ => unreachable!(),
                    },
                    _ => Err(BuckyError::new(
                        BuckyErrorCode::ErrorState,
                        "syn ack session data on error state",
                    )),
                },
                StreamStateImpl::Establish(state, _) => {
                    match state.provider.clone_as_package_handler() {
                        Some(h) => Ok(h),
                        None => Err(BuckyError::new(
                            BuckyErrorCode::InternalError,
                            "clone handler failed",
                        )),
                    }
                }
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "syn session data on error state",
                )),
            }?;
            handler.on_package(pkg, None)
        } else {
            trace!("{} on package {}", self, pkg);
            //进读锁转给provider
            let opt_handler: Option<Box<dyn OnPackage<SessionData>>> =
                match &*self.0.state.read().unwrap() {
                    StreamStateImpl::Establish(state, _) => state.provider.clone_as_package_handler(),
                    StreamStateImpl::Connecting(connecting, _) => match connecting {
                        StreamConnectingState::Connect(_) => None,
                        StreamConnectingState::Accept(acceptor) => {
                            Some(Box::new(acceptor.builder.clone()))
                        }
                    },
                    StreamStateImpl::Closing(state, _) => {
                        state.provider.clone_as_package_handler()
                        // unimplemented!()
                    }
                    _ => None,
                };
            match opt_handler {
                Some(provider) => provider.on_package(pkg, None),
                None => Ok(OnPackageResult::Handled),
            }
        }
    }
}

impl OnPackage<TcpSynConnection> for StreamContainer {
    fn on_package(
        &self,
        pkg: &TcpSynConnection,
        _: Option<()>,
    ) -> BuckyResult<OnPackageResult> {
        debug!("{} on package {}", self, pkg);
        assert_eq!(pkg.reverse_endpoint.is_some(), true);
        // syn tcp 直接转给builder
        let state = &*self.0.state.read().unwrap();
        let builder = match state {
            StreamStateImpl::Connecting(connecting, _) => match connecting {
                StreamConnectingState::Accept(acceptor) => Ok(acceptor.builder.clone()),
                _ => Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "tcp syn connection on error state",
                )),
            },
            _ => Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "tcp syn connection on error state",
            )),
        }?;
        builder.on_package(pkg, None)
    }
}

impl OnPackage<TcpAckConnection, tcp::AcceptInterface> for StreamContainer {
    fn on_package(
        &self,
        pkg: &TcpAckConnection,
        interface: tcp::AcceptInterface,
    ) -> BuckyResult<OnPackageResult> {
        debug!("{} on package {} from {}", self, pkg, interface);
        let opt_handler = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Connecting(connecting, _) => {
                    match connecting {
                        StreamConnectingState::Connect(connector) => {
                            match &connector.state {
                                connector::State::Builder(builder) => {
                                    Some(Box::new(builder.as_ref().clone())
                                        as Box<
                                            dyn OnPackage<TcpAckConnection, tcp::AcceptInterface>,
                                        >)
                                }
                                connector::State::Reverse(reverse) => {
                                    if reverse.action.local().eq(interface.local())
                                    /*&& reverse.action.remote().eq(interface.remote())*/
                                    {
                                        Some(Box::new(reverse.action.clone())
                                            as Box<
                                                dyn OnPackage<
                                                    TcpAckConnection,
                                                    tcp::AcceptInterface,
                                                >,
                                            >)
                                    } else {
                                        debug!(
                                            "{} ignore incoming stream {} for local is {}",
                                            self,
                                            interface,
                                            reverse.action.local()
                                        );
                                        None
                                    }
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        };
        opt_handler
            .ok_or_else(|| {
                BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "tcp ack connection on error state",
                )
            })
            .and_then(|handler| handler.on_package(pkg, interface.clone()))
            .map_err(|err| {
                let stream = self.clone();
                task::spawn(async move {
                    let ack_ack_stream = stream.ack_ack_tcp_stream(TCP_ACK_CONNECTION_RESULT_REFUSED);
                    let _ = match interface
                        .confirm_accept(vec![DynamicPackage::from(ack_ack_stream)])
                        .await
                    {
                        Ok(_) => {
                            debug!(
                                "{} confirm {} with refuse tcp connection ",
                                stream,
                                interface
                            );
                        }
                        Err(e) => {
                            warn!(
                                "{} confirm {} with tcp ack ack connection failed for {}",
                                stream,
                                interface,
                                e
                            );
                            let tunnel = stream.tunnel()
                                .ok_or_else(|| BuckyError::new(BuckyErrorCode::ErrorState, "stream's closed"))
                                .and_then(|tunnel| tunnel.create_tunnel::<tunnel::tcp::Tunnel>(
                                    EndpointPair::from((
                                        *interface.local(),
                                        Endpoint::default_tcp(interface.local()),
                                    )),
                                    ProxyType::None,
                                ));
                            if let Ok((tunnel, _)) = tunnel {
                                tunnel.mark_dead(tunnel.state());
                            }
                        }
                    };
                });
                err
            })
    }
}

struct StreamGuardImpl(StreamContainer);

impl Drop for StreamGuardImpl {
    fn drop(&mut self) {
        debug!("{} droped and will closed", self.0);

        let _ = self.0.shutdown(Shutdown::Both);
    }
}

#[derive(Clone)]
pub struct StreamGuard(Arc<StreamGuardImpl>);

impl fmt::Display for StreamGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StreamGuard {{stream:{}}}", (*self.0).0)
    }
}

impl Deref for StreamGuard {
    type Target = StreamContainer;
    fn deref(&self) -> &StreamContainer {
        &(*self.0).0
    }
}

impl From<StreamContainer> for StreamGuard {
    fn from(stream: StreamContainer) -> Self {
        Self(Arc::new(StreamGuardImpl(stream)))
    }
}

impl StreamGuard {
    pub fn display_ref_count(&self) {
        info!(
            "bdt stream ref counts: seq={:?}, impl=({}, {}), container=({},{})",
            self.sequence(),
            Arc::strong_count(&self.0),
            Arc::weak_count(&self.0),
            Arc::strong_count(&self.0 .0 .0),
            Arc::weak_count(&self.0 .0 .0)
        );
    }
}

impl Read for StreamGuard {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut &*self).poll_read(cx, buf)
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [std::io::IoSliceMut<'_>],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut &*self).poll_read_vectored(cx, bufs)
    }
}

impl Read for &StreamGuard {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let guard_impl = self.0.clone();
        let container = &guard_impl.0;
        container.poll_read(cx, buf)
    }
}

impl Write for StreamGuard {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut &*self).poll_write(cx, buf).map_err(|e| {
            error!("stream guard poll_write error: {}", e);
            e
        })
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut &*self)
            .poll_write_vectored(cx, bufs)
            .map_err(|e| {
                error!("{} poll_write_vectored error: {}", (*self), e);
                e
            })
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut &*self).poll_flush(cx).map_err(|e| {
            error!("{} poll_flush error: {}", (*self), e);
            e
        })
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut &*self).poll_close(cx).map_err(|e| {
            error!("{} poll_close error: {}", (*self), e);
            e
        })
    }
}

impl Write for &StreamGuard {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let guard_impl = self.0.clone();
        let container = &guard_impl.0;
        container.poll_write(cx, buf).map_err(|e| {
            error!("{} poll_write error: {}", (*self), e);
            e
        })
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let guard_impl = self.0.clone();
        let container = &guard_impl.0;
        container.poll_flush(cx).map_err(|e| {
            error!("{} poll_flush error: {}", (*self), e);
            e
        })
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let guard_impl = self.0.clone();
        let container = &guard_impl.0;
        container.poll_close(cx).map_err(|e| {
            error!("{} poll_close error: {}", (*self), e);
            e
        })
    }
}
