mod dep {
    pub use super::super::{
        package::PackageStream, stream_provider::StreamProvider, tcp::TcpStream,
    };
    pub use crate::{
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

const ANSWER_MAX_LEN: usize = 1380;

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
            debug!("{} connect with {}", stream.as_ref(), self);
            let action = ConnectTcpStream::new(
                stream.as_ref().stack.clone(),
                stream.clone(),
                self.0.clone(),
            );
            let stream = stream.clone();
            task::spawn(async move {
                match action.wait_pre_establish().await {
                    ConnectStreamState::PreEstablish => match action.continue_connect().await {
                        Ok(_) => {}
                        Err(err) => {
                            let _ = stream.as_ref().cancel_connecting_with(&err);
                        }
                    },
                    _ => {
                        let _ = stream.as_ref().cancel_connecting_with(&BuckyError::new(
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
            debug!("{} connect with {}", stream.as_ref(), self);
            let action = self.0.clone();
            let stream = stream.clone();
            task::spawn(async move {
                match action.wait_pre_establish().await {
                    ConnectStreamState::PreEstablish => match action.continue_connect().await {
                        Ok(_) => {}
                        Err(err) => {
                            let _ = stream.as_ref().cancel_connecting_with(&err);
                        }
                    },
                    _ => {
                        let _ = stream.as_ref().cancel_connecting_with(&BuckyError::new(
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
            debug!("{} connect with {}", stream.as_ref(), self);
            let provider = self.clone();
            let stream = stream.clone();
            let mut syn_tcp = stream.as_ref().syn_tcp_stream().unwrap();

            let stack = stream.as_ref().stack();
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
                        stream
                            .as_ref()
                            .stack()
                            .config()
                            .stream
                            .stream
                            .package
                            .connect_resend_interval,
                        provider.action.wait_pre_establish(),
                    )
                    .await
                    {
                        Err(_) => {
                            let _ = stream
                                .as_ref()
                                .tunnel()
                                .send_packages(vec![DynamicPackage::from(syn_tcp.clone())]);
                        }
                        Ok(state) => {
                            match state {
                                ConnectStreamState::PreEstablish => {
                                    match provider.action.continue_connect().await {
                                        Ok(_) => {}
                                        Err(err) => {
                                            let _ = stream.as_ref().cancel_connecting_with(&err);
                                        }
                                    }
                                }
                                _ => {
                                    let _ =
                                        stream.as_ref().cancel_connecting_with(&BuckyError::new(
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

#[derive(Eq, PartialEq)]
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
    Initial,
    Connecting(StreamConnectingState),
    Establish(StreamEstablishState),
    Closing(StreamEstablishState),
    Closed,
}

struct PackageStreamProviderSelector {}
pub enum StreamProviderSelector {
    Package(IncreaseId /*remote id*/, Option<SessionData>),
    Tcp(async_std::net::TcpStream, AesKey, Option<TcpAckConnection>),
}

pub struct StreamContainerImpl {
    stack: WeakStack,
    tunnel: TunnelGuard,
    remote_port: u16,
    local_id: IncreaseId,
    sequence: TempSeq,
    state: RwLock<StreamStateImpl>,
    answer_data: RwLock<Option<Vec<u8>>>,
}

impl fmt::Display for StreamContainerImpl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StreamContainer {{sequence:{:?}, local:{}, remote:{}, port:{}, id:{} }}",
            &self.sequence,
            Stack::from(&self.stack).local_device_id(),
            self.tunnel.remote(),
            &self.remote_port,
            &self.local_id
        )
    }
}

impl StreamContainerImpl {
    pub fn new(
        weak_stack: WeakStack,
        tunnel: TunnelGuard,
        remote_port: u16,
        local_id: IncreaseId,
        sequence: TempSeq,
    ) -> StreamContainer {
        let stream_impl = StreamContainerImpl {
            stack: weak_stack,
            tunnel: tunnel,
            remote_port,
            local_id,
            sequence: sequence,
            state: RwLock::new(StreamStateImpl::Initial),
            answer_data: RwLock::new(None)
        };

        StreamContainer(Arc::new(stream_impl))
    }

    //收到ack以后继续连接, 可以完成时在builder里面调用 establish_with
    pub fn accept(&self, arc_self: &StreamContainer, remote_id: IncreaseId) {
        let state = &mut *self.state.write().unwrap();
        if let StreamStateImpl::Initial = *state {
            info!("{} initial=>accepting remote_id {}", self, remote_id);
            *state =
                StreamStateImpl::Connecting(StreamConnectingState::Accept(acceptor::Acceptor {
                    remote_id,
                    waiter: StateWaiter::new(),
                    builder: AcceptStreamBuilder::new(self.stack.clone(), arc_self.clone()),
                }));

            let stream = arc_self.clone();
            task::spawn(async move {
                match future::timeout(
                    stream
                        .as_ref()
                        .stack()
                        .config()
                        .stream
                        .stream
                        .connect_timeout,
                    stream.as_ref().wait_establish(),
                )
                .await
                {
                    Ok(r) => r,
                    Err(err) => {
                        let err = err.into();
                        match stream.as_ref().cancel_connecting_with(&err) {
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
    pub async fn connect(
        &self,
        arc_self: &StreamContainer,
        question: Vec<u8>,
        build_params: BuildTunnelParams,
    ) -> Result<(), BuckyError> {
        // initial connector
        let connector = connector::Connector {
            question: Vec::from(question),
            waiter: StateWaiter::new(),
            state: connector::State::Unknown,
            start_at: bucky_time_now(),
        };

        // enter connecting connector unknown
        {
            let mut state = self.state.write().unwrap();
            if let StreamStateImpl::Initial = *state {
                info!("{} initial=>connecting", self);
                *state = StreamStateImpl::Connecting(StreamConnectingState::Connect(connector));
            } else {
                unreachable!("not initial")
            }
        }

        // 从tunnel container返回要用的connector provider
        let _ = match self
            .tunnel
            .select_stream_connector(build_params, arc_self.clone())
            .await
        {
            Ok(selector) => {
                let (connector_state, connector_provider) =
                    connector::State::from((arc_self, selector));
                // enter connecting connector provider
                {
                    let state = &mut *self.state.write().unwrap();
                    if let StreamStateImpl::Connecting(ref mut connecting) = state {
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
                connector_provider.begin_connect(arc_self);
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
    pub async fn establish_with(
        &self,
        selector: StreamProviderSelector,
        arc_self: &StreamContainer,
    ) -> Result<(), BuckyError> {
        let remote_timestamp = match self.tunnel().wait_active().await {
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

                let stream = PackageStream::new(self, self.local_id.clone(), remote_id)?;
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

                let stream = TcpStream::new(arc_self.clone(), socket, key)?;
                (
                    Box::new(stream.clone()) as Box<dyn StreamProvider>,
                    Box::new(stream) as Box<dyn StreamProvider>,
                    answer_data,
                )
            }
        };

        let state = &mut *self.state.write().unwrap();
        let waiter = match state {
            StreamStateImpl::Connecting(ref mut connecting_state) => match connecting_state {
                StreamConnectingState::Accept(ref mut acceptor) => {
                    let waiter = acceptor.waiter.transfer();
                    info!("{} accepting=>establish with provider {}", self, provider);
                    *state = StreamStateImpl::Establish(StreamEstablishState {
                        start_at: bucky_time_now(),
                        remote_timestamp,
                        provider: provider_stub,
                    });
                    Ok(waiter)
                }
                StreamConnectingState::Connect(ref mut connector) => {
                    let waiter = connector.waiter.transfer();
                    info!("{} connecting=>establish with {}", self, provider);
                    *state = StreamStateImpl::Establish(StreamEstablishState {
                        start_at: bucky_time_now(),
                        remote_timestamp,
                        provider: provider_stub,
                    });

                    if answer_data.len() > 0 {
                        let data = &mut *self.answer_data.write().unwrap();
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
        provider.start(arc_self);
        // 唤醒等待 establish 的waiter
        waiter.wake();
        Ok(())
    }

    pub fn cancel_connecting_with(&self, err: &BuckyError) -> Result<(), BuckyError> {
        warn!("{} cancel connecting with error: {}", self, err);
        let state = &mut *self.state.write().unwrap();
        let (waiter, state_dump) = match state {
            StreamStateImpl::Connecting(ref mut connecting_state) => match connecting_state {
                StreamConnectingState::Accept(ref mut acceptor) => {
                    let waiter = acceptor.waiter.transfer();
                    info!("{} accepting=>closed", self);
                    *state = StreamStateImpl::Closed;
                    Ok((waiter, None))
                }
                StreamConnectingState::Connect(ref mut connector) => {
                    let waiter = connector.waiter.transfer();
                    info!("{} connecting=>closed", self);
                    let state_dump = connector
                        .state
                        .remote_timestamp()
                        .map(|r| (r, connector.start_at));
                    *state = StreamStateImpl::Closed;
                    Ok((waiter, state_dump))
                }
            },
            _ => Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "stream not connecting",
            )),
        }?;
        if let Some((remote_timestamp, start_at)) = state_dump {
            error!("{} mark tunnel dead", self);
            let _ = self.tunnel.mark_dead(remote_timestamp, start_at);
        }
        // 唤醒等待 establish 的waiter
        waiter.wake();
        Ok(())
    }

    pub async fn wait_establish(&self) -> Result<(), BuckyError> {
        let waiter = {
            let state = &mut *self.state.write().unwrap();
            match state {
                StreamStateImpl::Connecting(ref mut connecting) => {
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
                StreamStateImpl::Establish(_) => Ok(None),
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

    pub fn syn_session_data(&self) -> Option<SessionData> {
        {
            match &*self.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting) => match connecting {
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
                sequence: self.sequence,
                from_session_id: self.local_id.clone(),
                to_vport: self.remote_port,
            });
            session.session_id = self.local_id.clone();
            session.send_time = bucky_time_now();
            session.flags_add(SESSIONDATA_FLAG_SYN);
            session.payload = TailedOwnedData::from(question);
            session
        })
    }

    pub fn syn_ack_session_data(&self, answer: &[u8]) -> Option<SessionData> {
        {
            match &*self.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting) => match connecting {
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
                sequence: self.sequence,
                from_session_id: self.local_id.clone(),
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

    pub fn syn_tcp_stream(&self) -> Option<TcpSynConnection> {
        {
            match &*self.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting) => match connecting {
                    StreamConnectingState::Connect(connector) => Some(connector.question.clone()),
                    _ => {
                        unreachable!()
                    }
                },
                _ => None,
            }
        }
        .map(|question| {
            let local_device = Stack::from(&self.stack).local().clone();
            TcpSynConnection {
                sequence: self.sequence,
                result: 0u8,
                to_vport: self.remote_port,
                from_session_id: self.local_id,
                from_device_id: local_device.desc().device_id(),
                from_device_desc: local_device,
                to_device_id: self.tunnel().remote().clone(),
                reverse_endpoint: None,
                payload: TailedOwnedData::from(question),
            }
        })
    }

    pub fn ack_tcp_stream(&self, answer: &[u8]) -> Option<TcpAckConnection> {
        {
            match &*self.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting) => match connecting {
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
                sequence: self.sequence,
                to_session_id: remote_id,
                result: TCP_ACK_CONNECTION_RESULT_OK,
                to_device_desc: Stack::from(&self.stack).local().clone(),
                payload: TailedOwnedData::from(payload),
            }
        })
    }

    pub fn ack_ack_tcp_stream(&self, result: u8) -> TcpAckAckConnection {
        TcpAckAckConnection {
            sequence: self.sequence,
            result,
        }
    }

    pub fn tunnel(&self) -> &TunnelContainer {
        &self.tunnel
    }

    pub fn state(&self) -> StreamState {
        match &*self.state.read().unwrap() {
            StreamStateImpl::Initial => unreachable!(),
            StreamStateImpl::Connecting(_) => StreamState::Connecting,
            StreamStateImpl::Establish(establish) => {
                StreamState::Establish(establish.remote_timestamp)
            }
            StreamStateImpl::Closing(_) => StreamState::Closing,
            StreamStateImpl::Closed => StreamState::Closed,
        }
    }

    pub fn is_connecting(&self) -> bool {
        let state = self.state.read().unwrap();
        let s1 = state.deref();
        match s1 {
            StreamStateImpl::Connecting(_) => true,
            _ => false,
        }
    }

    pub fn acceptor(&self) -> Option<AcceptStreamBuilder> {
        if let StreamStateImpl::Connecting(connecting) = &*self.state.read().unwrap() {
            if let StreamConnectingState::Accept(acceptor) = connecting {
                return Some(acceptor.builder.clone());
            }
        }
        None
    }

    pub fn stack(&self) -> Stack {
        Stack::from(&self.stack)
    }

    pub fn break_with_error(&self, arc_self: &StreamContainer, err: BuckyError) {
        error!("{} break with err {}", self, err);
        let state_dump = {
            let state = &mut *self.state.write().unwrap();
            match state {
                StreamStateImpl::Establish(establish) => {
                    let state_dump = Some((establish.remote_timestamp, establish.start_at));
                    *state = StreamStateImpl::Closed;
                    state_dump
                }
                _ => None,
            }
        };
        if let Some((remote_timestamp, start_at)) = state_dump {
            debug!("{} mark tunnel dead for break", self);
            let _ = self.tunnel().mark_dead(remote_timestamp, start_at);
        }
        self.stack().stream_manager().remove_stream(arc_self);
    }

    pub fn on_shutdown(&self, arc_self: &StreamContainer) {
        *self.state.write().unwrap() = StreamStateImpl::Closed;
        self.stack().stream_manager().remove_stream(arc_self);
    }
}

#[derive(Clone)]
pub struct StreamContainer(Arc<StreamContainerImpl>);

impl StreamContainer {
    pub async fn confirm(&self, answer: &[u8]) -> Result<(), BuckyError> {
        if answer.len() > ANSWER_MAX_LEN {
            return Err(BuckyError::new(
                BuckyErrorCode::Failed,
                format!("answer's length large than {}", ANSWER_MAX_LEN),
            ));
        }

        let builder = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Connecting(connecting) => match connecting {
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
                error!("{} confirm failed for {}", self.as_ref(), &e);
                Err(e)
            }
            Ok(v) => {
                info!("{} confirmed", self.as_ref());
                Ok(v)
            }
        }
    }

    pub fn shutdown(&self, which: Shutdown) -> Result<(), std::io::Error> {
        info!("{} shutdown", self.as_ref());
        let provider = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Establish(s) => Ok(s.provider.clone_as_provider()),
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
            error!("{} shutdown failed for {}", self.as_ref(), e);
            e
        })?;
        provider.shutdown(which, &self)
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
                StreamStateImpl::Establish(s) => Some(s.provider.clone_as_provider()),
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

    fn contain_answer_data(&self) -> bool {
        match *self.0.answer_data.read().unwrap() {
            None => false,
            _ => true,
        }
    }

    fn answer_read(&self, buf: &mut [u8]) -> usize {
        if self.contain_answer_data() {
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
        debug!("{} poll read {} bytes", self.as_ref(), buf.len());

        self.poll_io_wait_establish(cx.waker().clone(), true, |provider| {
            let read_len = self.answer_read(buf);

            if read_len > 0 {
                Poll::Ready(Ok(read_len))
            } else {
                provider.poll_read(cx, buf)
            }
        })
    }

    fn poll_write(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        debug!("{} poll write {} bytes", self.as_ref(), buf.len());
        self.poll_io_wait_establish(cx.waker().clone(), false, |provider| {
            provider.poll_write(cx, buf)
        })
    }

    fn poll_flush(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        debug!("{} poll flush", self.as_ref());
        self.poll_io_wait_establish(cx.waker().clone(), false, |provider| {
            provider.poll_flush(cx)
        })
    }

    fn poll_close(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        debug!("{} poll close", self.as_ref());
        let provider = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Establish(s) => Some(s.provider.clone_as_provider()),
                StreamStateImpl::Initial => {
                    debug!("poll-close, {} in initial.", self.as_ref());
                    None
                }
                StreamStateImpl::Connecting(_) => {
                    debug!("poll-close, {} in connecting.", self.as_ref());
                    None
                }
                StreamStateImpl::Closing(s) => {
                    debug!("poll-close, {} in closing.", self.as_ref());
                    Some(s.provider.clone_as_provider())
                }
                StreamStateImpl::Closed => {
                    debug!("poll-close, {} in closed ready.", self.as_ref());
                    return Poll::Ready(Ok(()));
                }
            }
        };

        match provider {
            Some(provider) => provider.poll_close(cx),
            None => {
                let _ = self.0.cancel_connecting_with(&BuckyError::new(
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
        (self.0.tunnel.remote(), self.0.remote_port)
    }

    pub fn sequence(&self) -> TempSeq {
        self.0.sequence
    }

    pub fn state(&self) -> StreamState {
        self.0.state()
    }

    pub fn local_id(&self) -> IncreaseId {
        self.0.local_id
    }

    pub fn local_ep(&self) -> Option<Endpoint> {
        let state = &*self.0.state.read().unwrap();
        match state {
            StreamStateImpl::Establish(s) => Some(*s.provider.local_ep()),
            _ => None,
        }
    }

    pub fn remote_ep(&self) -> Option<Endpoint> {
        let state = &*self.0.state.read().unwrap();
        match state {
            StreamStateImpl::Establish(s) => Some(*s.provider.remote_ep()),
            _ => None,
        }
    }

    fn poll_io_wait_establish<R>(
        &self,
        waker: Waker,
        is_read: bool,
        mut proc: impl FnMut(&dyn StreamProvider) -> Poll<std::io::Result<R>>,
    ) -> Poll<std::io::Result<R>> {
        let provider = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Establish(s) => Some(s.provider.clone_as_provider()),
                StreamStateImpl::Initial | StreamStateImpl::Connecting(_) => {
                    trace!(
                        "{} poll-io(read:{}) in initial/connecting.",
                        self.as_ref(),
                        is_read
                    );
                    None
                }
                StreamStateImpl::Closing(s) if is_read => {
                    trace!("{} poll-io(read:{}) in closing.", self.as_ref(), is_read);
                    Some(s.provider.clone_as_provider())
                }
                _ => {
                    let msg = format!("{} poll-io(read:{}) in closed.", self.as_ref(), is_read);
                    error!("{}", msg);
                    return Poll::Ready(Err(std::io::Error::new(ErrorKind::NotConnected, msg)));
                }
            }
        };

        match provider {
            Some(provider) => proc(&*provider),
            None => {
                let container_impl = self.0.clone();
                task::spawn(async move {
                    let _ = container_impl.wait_establish().await;
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

impl AsRef<StreamContainerImpl> for StreamContainer {
    fn as_ref(&self) -> &StreamContainerImpl {
        &self.0
    }
}

impl OnPackage<TcpSynConnection, tcp::AcceptInterface> for StreamContainer {
    fn on_package(
        &self,
        pkg: &TcpSynConnection,
        interface: tcp::AcceptInterface,
    ) -> Result<OnPackageResult, BuckyError> {
        debug!("{} on package {} from {}", self.as_ref(), pkg, interface);
        // syn tcp 直接转给builder
        let state = &*self.0.state.read().unwrap();
        let builder = match state {
            StreamStateImpl::Connecting(connecting) => match connecting {
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
    fn on_package(&self, pkg: &SessionData, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        if pkg.is_syn() {
            debug!("{} on package {}", self.as_ref(), pkg);
            // syn session data直接转给builder
            let state = &*self.0.state.read().unwrap();
            let builder = match state {
                StreamStateImpl::Connecting(connecting) => match connecting {
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
            debug!("{} on package {}", self.as_ref(), pkg);
            // 传给 connector provider
            let handler: Box<dyn OnPackage<SessionData>> = match &*self.0.state.read().unwrap() {
                StreamStateImpl::Connecting(connecting) => match connecting {
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
                StreamStateImpl::Establish(state) => {
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
            trace!("{} on package {}", self.as_ref(), pkg);
            //进读锁转给provider
            let opt_handler: Option<Box<dyn OnPackage<SessionData>>> =
                match &*self.0.state.read().unwrap() {
                    StreamStateImpl::Establish(state) => state.provider.clone_as_package_handler(),
                    StreamStateImpl::Connecting(connecting) => match connecting {
                        StreamConnectingState::Connect(_) => None,
                        StreamConnectingState::Accept(acceptor) => {
                            Some(Box::new(acceptor.builder.clone()))
                        }
                    },
                    StreamStateImpl::Closing(state) => {
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
    ) -> Result<OnPackageResult, BuckyError> {
        debug!("{} on package {}", self.as_ref(), pkg);
        assert_eq!(pkg.reverse_endpoint.is_some(), true);
        // syn tcp 直接转给builder
        let state = &*self.0.state.read().unwrap();
        let builder = match state {
            StreamStateImpl::Connecting(connecting) => match connecting {
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
    ) -> Result<OnPackageResult, BuckyError> {
        debug!("{} on package {} from {}", self.as_ref(), pkg, interface);
        let opt_handler = {
            let state = &*self.0.state.read().unwrap();
            match state {
                StreamStateImpl::Connecting(connecting) => {
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
                                            self.as_ref(),
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
                    let ack_ack_stream = stream
                        .as_ref()
                        .ack_ack_tcp_stream(TCP_ACK_CONNECTION_RESULT_REFUSED);
                    let _ = match interface
                        .confirm_accept(vec![DynamicPackage::from(ack_ack_stream)])
                        .await
                    {
                        Ok(_) => {
                            debug!(
                                "{} confirm {} with refuse tcp connection ",
                                stream.as_ref(),
                                interface
                            );
                        }
                        Err(e) => {
                            warn!(
                                "{} confirm {} with tcp ack ack connection failed for {}",
                                stream.as_ref(),
                                interface,
                                e
                            );
                            let tunnel: BuckyResult<tunnel::tcp::Tunnel> =
                                stream.as_ref().tunnel().create_tunnel(
                                    EndpointPair::from((
                                        *interface.local(),
                                        Endpoint::default_tcp(interface.local()),
                                    )),
                                    ProxyType::None,
                                );
                            if let Ok(tunnel) = tunnel {
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
        debug!("{} droped and will closed", self.0.as_ref());

        let _ = self.0.shutdown(Shutdown::Both);
    }
}

#[derive(Clone)]
pub struct StreamGuard(Arc<StreamGuardImpl>);

impl fmt::Display for StreamGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StreamGuard {{stream:{}}}", (*self.0).0.as_ref())
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
                error!("{} poll_write_vectored error: {}", (*self).as_ref(), e);
                e
            })
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut &*self).poll_flush(cx).map_err(|e| {
            error!("{} poll_flush error: {}", (*self).as_ref(), e);
            e
        })
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut &*self).poll_close(cx).map_err(|e| {
            error!("{} poll_close error: {}", (*self).as_ref(), e);
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
            error!("{} poll_write error: {}", (*self).as_ref(), e);
            e
        })
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let guard_impl = self.0.clone();
        let container = &guard_impl.0;
        container.poll_flush(cx).map_err(|e| {
            error!("{} poll_flush error: {}", (*self).as_ref(), e);
            e
        })
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let guard_impl = self.0.clone();
        let container = &guard_impl.0;
        container.poll_close(cx).map_err(|e| {
            error!("{} poll_close error: {}", (*self).as_ref(), e);
            e
        })
    }
}
