use log::*;
use std::{
    collections::{LinkedList, BTreeMap}, 
    time::Duration, 
    task::{Context, Poll, Waker}, 
    pin::Pin, 
    sync::{atomic::{AtomicBool, Ordering}}, 
    ops::Deref, 
    net::Shutdown, 
};
use cyfs_debug::Mutex;
use async_std::{
    future, 
    sync::{Arc}, 
    task, 
    channel::{bounded, Sender, Receiver}, 
    io::prelude::{Read, Write}
};
use futures::StreamExt;

use cyfs_base::*;
use crate::{
    types::*, 
    tunnel::{BuildTunnelParams, TunnelState}, 
    stream::{StreamGuard, StreamContainer, StreamState, StreamListenerGuard},  
    stack::{Stack}
};


#[derive(Clone)]
pub struct PooledStream(Arc<PooledStreamImpl>);

impl std::fmt::Display for PooledStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PooledStream {{stream:{}}}", self.0.stream.as_ref())
    }
}


impl Deref for PooledStream {
    type Target = StreamContainer;
    fn deref(&self) -> &StreamContainer {
        &self.0.stream
    }
}

enum PooledStreamType {
    Active(StreamPoolConnector), 
    Passive(StreamPoolListener), 
}

struct PooledStreamImpl {
    shutdown: AtomicBool, 
    stream_type: PooledStreamType, 
    stream: StreamGuard, 
}

impl PooledStream {
    pub fn shutdown(&self, which: Shutdown) -> std::io::Result<()> {
        self.0.shutdown.store(true, Ordering::SeqCst);
        self.0.stream.shutdown(which)
    }
}

impl Drop for PooledStreamImpl {
    fn drop(&mut self) {
        let shutdown = self.shutdown.load(Ordering::SeqCst);
        match &self.stream_type {
            PooledStreamType::Passive(owner) => owner.recycle(&self.stream, shutdown), 
            PooledStreamType::Active(owner) => owner.recycle(&self.stream, shutdown)
        };
    }
}


impl Read for PooledStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut stream = self.0.stream.clone();
        Pin::new(&mut stream).poll_read(cx, buf)
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [std::io::IoSliceMut<'_>],
    ) -> Poll<std::io::Result<usize>> {
        let mut stream = self.0.stream.clone();
        Pin::new(&mut stream).poll_read_vectored(cx, bufs)
    }
}


impl Write for PooledStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut stream = self.0.stream.clone();
        Pin::new(&mut stream).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        let mut stream = self.0.stream.clone();
        Pin::new(&mut stream).poll_write_vectored(cx, bufs)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let mut stream = self.0.stream.clone();
        Pin::new(&mut stream).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let mut stream = self.0.stream.clone();
        Pin::new(&mut stream).poll_close(cx)
    }
}


#[derive(Clone)]
struct StreamPoolConnector(Arc<StreamPoolConnectorImpl>);

impl std::fmt::Display for StreamPoolConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamPoolConnector {{local:{} remote:{} port:{}}}", self.0.stack.local_device_id(), self.0.remote, self.0.port)
    }
}

struct StreamPoolConnectorImpl {
    stack: Stack, 
    remote: DeviceId, 
    port: u16, 
    capacity: usize, 
    timeout: Duration, 
    stream_list: Mutex<LinkedList<(StreamGuard, Timestamp)>>,
}

impl StreamPoolConnector {
    pub fn new(
        stack: Stack, 
        remote: &DeviceId, 
        port: u16, 
        capacity: usize, 
        timeout: Duration
    ) -> Self {
        Self(Arc::new(StreamPoolConnectorImpl {
            stack, 
            remote: remote.clone(), 
            port, 
            capacity, 
            timeout, 
            stream_list: Mutex::new(LinkedList::new()), 
        }))
    }

    pub fn stream_count(&self) -> usize {
        self.0.stream_list.lock().unwrap().len()
    }

    pub fn remote(&self) -> (&DeviceId, u16) {
        (&self.0.remote, self.0.port)
    }

    fn wrap_stream(&self, stream: StreamGuard) -> PooledStream {
        PooledStream(Arc::new(PooledStreamImpl {
            shutdown: AtomicBool::new(false), 
            stream_type: PooledStreamType::Active(self.clone()), 
            stream, 
        }))
    }

    pub async fn connect(&self) -> BuckyResult<PooledStream> {
        let exists = {
            let mut stream_list = self.0.stream_list.lock().unwrap();
            stream_list.pop_front()
        };
        if let Some((stream, _)) = exists {
            debug!("{} connect return reused stream {}", self, stream);
            Ok(self.wrap_stream(stream))
        } else {
            debug!("{} will connect new stream", self);
            let stack = &self.0.stack;
            if let Some(remote_device) = stack.device_cache().get(&self.0.remote).await {
                let build_params = BuildTunnelParams {
                    remote_const: remote_device.desc().clone(),
                    remote_sn: None,
                    remote_desc: Some(remote_device)
                };
                let stream = stack.stream_manager().connect(
                    self.0.port, 
                    vec![], 
                    build_params).await
                    .map_err(|e| {
                        warn!("{} connect new stream failed for {}", self, e);
                        e
                    })?;
                info!("{} return newly connected stream {}", self, stream);
                Ok(self.wrap_stream(stream))
            } else {
                let e = BuckyError::new(BuckyErrorCode::NotFound, "device desc not cached");
                warn!("{} connect failed for {}", self, e);
                Err(e)
            }
        }
    }


    fn recycle(&self, stream: &StreamGuard, shutdown: bool) {
        debug!("{} will recycle stream {}", self, stream);
        if let StreamState::Establish(_) = stream.state() {
            if !shutdown {
                let mut stream_list = self.0.stream_list.lock().unwrap();
                if stream_list.len() < self.0.capacity {
                    stream_list.push_back((stream.clone(), bucky_time_now()));
                } else {
                    warn!("{} drop stream {} for full", self, stream);
                }   
            } else {
                self.check_tunnel();
                info!("{} drop stream {} for shutdown", self, stream);
            }
        } else {
            self.check_tunnel();
            warn!("{} drop stream {} for not establish", self, stream);
        }
    }

    fn drop_stream(&self, remote_timestamp: Option<Timestamp>) {
        let remove = if let Some(remote_timestamp) = remote_timestamp {
            let mut remain = LinkedList::new();
            let mut remove = LinkedList::new();
            let mut streams = self.0.stream_list.lock().unwrap();
            while let Some((stream, last_used)) = streams.pop_back() {
                match stream.state() {
                    StreamState::Establish(remote) => {
                        if remote <= remote_timestamp {
                            remove.push_back((stream, last_used));
                        } else {
                            remain.push_back((stream, last_used));
                        }
                    },
                    _ => {
                        remove.push_back((stream, last_used));
                    }
                }
            }
            *streams = remain;
            remove
        } else {
            let mut remove = LinkedList::new();
            let mut streams = self.0.stream_list.lock().unwrap();
            remove.append(&mut *streams);
            remove
        };
        
        for (stream, _) in remove {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }

    fn check_tunnel(&self) {
        if let Some(tunnel) = self.0.stack.tunnel_manager().container_of(self.remote().0) {
            let state = tunnel.state();
            match state {
                TunnelState::Active(remote) => {
                    self.drop_stream(Some(remote));
                }, 
                TunnelState::Dead => {
                    self.drop_stream(None);
                }, 
                _ => {}
            }
        } 
    }

    fn on_time_escape(&self, now: Timestamp) {
        let remove = {
            let mut remain = LinkedList::new();
            let mut remove = LinkedList::new();
            let mut streams = self.0.stream_list.lock().unwrap();
            while let Some((stream, last_used)) = streams.pop_front() {
                match stream.state() {
                    StreamState::Establish(_) => {
                        if now > last_used && Duration::from_micros(now - last_used) > self.0.timeout {
                            remove.push_back(stream);
                        } else {
                            remain.push_back((stream, last_used));
                        }
                    },
                    _ => {
                        remove.push_back(stream);
                    }
                }
            }
            *streams = remain;
            remove
        };
        
        for stream in remove {
            info!("{} shutdown stream {} for pool timeout", self, stream);
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

#[derive(Clone)]
struct StreamPoolListener(Arc<StreamPoolListenerImpl>);

struct StreamPoolListenerImpl {
    origin_listener: StreamListenerGuard, 
    sender: Sender<StreamGuard>, 
    recver: Receiver<StreamGuard>
}

impl std::fmt::Display for StreamPoolListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamPoolListener {{listener:{}}}", self.0.origin_listener)
    }
}

impl StreamPoolListener {
    pub fn new(origin_listener: StreamListenerGuard, backlog: usize) -> Self {
        let (sender, recver) = bounded::<StreamGuard>(backlog);
        
        let listener = Self(Arc::new(StreamPoolListenerImpl {
            origin_listener, 
            sender, 
            recver
        }));

        {
            let listener = listener.clone();
            task::spawn(async move {
                listener.listen().await;
            });
        }

        listener
    }

    async fn listen(&self) {
        info!("{} listen", self);
        let mut incoming = self.0.origin_listener.incoming();
        let listener = self.clone();
        loop {
            match incoming.next().await {
                Some(ret) => {
                    match ret {
                        Ok(pre_stream) => {
                            info!("{} accept stream {}", listener, pre_stream.stream);
                            let listener = self.clone();
                            task::spawn(async move {
                                match pre_stream.stream.confirm(b"".as_ref()).await {
                                    Ok(_) => {
                                        debug!("{} confirm stream {}", listener, pre_stream.stream);
                                        let _ = listener.0.sender.try_send(pre_stream.stream);
                                    },
                                    Err(e) => {
                                        error!("{} confirm stream {} failed for {}", listener, pre_stream.stream, e);
                                    }
                                }
                            });
                        }, 
                        Err(e) => {
                            error!("{} stop listen for {}", listener, e);
                            break;
                        }
                    }
                }, 
                None => {
                    // do nothing
                }
            }
        }
    } 

    fn wrap_stream(&self, stream: StreamGuard) -> PooledStream {
        PooledStream(Arc::new(PooledStreamImpl {
            shutdown: AtomicBool::new(false), 
            stream_type: PooledStreamType::Passive(self.clone()), 
            stream, 
        }))
    }

    pub async fn accept(&self) -> BuckyResult<PooledStream> {
        match self.0.recver.recv().await {
            Ok(stream) => {
                debug!("{} accepted stream {}", self, stream);
                Ok(self.wrap_stream(stream))
            },
            Err(_) => unreachable!()
        }
    }

    pub fn incoming(&self) -> PooledStreamIncoming {
        PooledStreamIncoming {
            owner: self.clone(), 
            state: Arc::new(Mutex::new(IncommingState {
                exists: None, 
                waker: None
            }))
        }
    }

    pub fn recycle(&self, stream: &StreamGuard, shutdown: bool) {
        if !shutdown {
            debug!("{} recyle stream {}", self, stream);
            let pool = self.clone();
            let stream = stream.clone();
            task::spawn(async move {
                match stream.readable().await {
                    Ok(len) => {
                        if len != 0 {
                            debug!("{} return resued stream {}", pool, stream);
                            let _ = pool.0.sender.try_send(stream);
                        } else {
                            // do nothing
                            debug!("{} drop stream {} for remote closed", pool, stream);
                        }
                    }, 
                    Err(e) => {
                        // do nothing
                        warn!("{} drop stream {} for {}", pool, stream, e);
                    }
                }
            });
        }
    }
}


struct IncommingState {
    exists: Option<std::io::Result<PooledStream>>,
    waker: Option<Waker>,
}

pub struct PooledStreamIncoming {
    owner: StreamPoolListener, 
    state: Arc<Mutex<IncommingState>>
}

impl async_std::stream::Stream for PooledStreamIncoming {
    type Item = std::io::Result<PooledStream>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<<Self as async_std::stream::Stream>::Item>> {
        let exists = {
            let mut state = self.state.lock().unwrap();
            match &state.exists {
                Some(exists) => {
                    match exists {
                        Ok(stream) => {
                            let exists = Ok(stream.clone());
                            state.exists = None;
                            Some(exists)
                        }, 
                        Err(_) => {
                            Some(Err(std::io::Error::new(std::io::ErrorKind::Other, BuckyError::new(BuckyErrorCode::ErrorState, "listener stopped"))))
                        }
                    }
                }, 
                None => {
                    assert!(state.waker.is_none());
                    state.waker = Some(cx.waker().clone());
                    None
                }
            }
        };

        if exists.is_none() {
            let owner = self.owner.clone();
            let state = self.state.clone();
            task::spawn(async move {
                let next = owner.accept().await
                    .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, BuckyError::new(BuckyErrorCode::ErrorState, "listener stopped")));
                let waker = {
                    let mut state = state.lock().unwrap();
                    assert!(state.waker.is_some());
                    assert!(state.exists.is_none());
                    state.exists = Some(next);
                    let mut waker = None;
                    std::mem::swap(&mut state.waker, &mut waker);
                    waker.unwrap()
                };
                waker.wake();
            });
            Poll::Pending
        } else {
            Poll::Ready(exists)
        }
    }
}


struct StreamPoolImpl {
    stack: Stack, 
    port: u16, 
    config: StreamPoolConfig, 
    connectors: Mutex<BTreeMap<DeviceId, StreamPoolConnector>>, 
    listener: StreamPoolListener
}

#[derive(Clone)]
pub struct StreamPool(Arc<StreamPoolImpl>);

impl std::fmt::Display for StreamPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamPool {{local:{} port:{}}}", self.0.stack.local_device_id(), self.port())
    }
}


#[derive(Debug)]
pub struct StreamPoolConfig {
    pub capacity: usize,
    pub backlog: usize,
    pub atomic_interval: Duration,
    pub timeout: Duration,
} 

impl Default for StreamPoolConfig {
    fn default() -> Self {
        Self {
            capacity: 10, 
            backlog: 100,
            atomic_interval: Duration::from_secs(5),  
            timeout: Duration::from_secs(30), 
        }
    }
}

impl StreamPool {
    pub fn new(
        stack: Stack, 
        port: u16, 
        config: StreamPoolConfig
    ) -> BuckyResult<Self> {
        info!("create stream pool on port {} config {:?}", port, config);
        let origin_listener = stack.stream_manager().listen(port)?;
        let listener = StreamPoolListener::new(origin_listener, config.backlog);

        let pool = Self(Arc::new(StreamPoolImpl {
            stack: stack.clone(), 
            port, 
            config, 
            connectors: Mutex::new(BTreeMap::new()), 
            listener
        }));

        {
            let pool = pool.clone();
            task::spawn(async move {
                let _ = future::timeout(pool.config().atomic_interval, future::pending::<()>()).await;
                pool.on_time_escape(bucky_time_now());
            });
        }

        Ok(pool)
    }

    fn on_time_escape(&self, now: Timestamp) {
        let connectors: Vec<StreamPoolConnector> = self.0.connectors.lock().unwrap().values().cloned().collect();

        for connector in connectors {
            connector.on_time_escape(now);
        }
    }

    pub async fn connect(&self, remote: &DeviceId) -> BuckyResult<PooledStream> {
        debug!("{} will connect to {}", self, remote);
        let connector = {
            let mut connectors = self.0.connectors.lock().unwrap();
            if let Some(connector) = connectors.get(remote) {
                connector.clone()
            } else {
                let connector = StreamPoolConnector::new(self.0.stack.clone(), remote, self.port(), self.config().capacity, self.config().timeout);
                connectors.insert(remote.clone(), connector.clone());
                connector
            }
        };
        connector.connect().await
    }

    pub fn incoming(&self) -> PooledStreamIncoming {
        self.0.listener.incoming()
    }

    pub fn port(&self) -> u16 {
        self.0.port
    }

    pub fn config(&self) -> &StreamPoolConfig {
        &self.0.config
    }
}