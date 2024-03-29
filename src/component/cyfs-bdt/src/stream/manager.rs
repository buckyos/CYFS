use async_std::sync::{Arc, Weak};
use std::{
    collections::{BTreeMap}, 
    sync::{RwLock, Mutex}
};
use lru_time_cache::LruCache;
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*},
    interface::*,  
    tunnel::{TunnelGuard, TunnelContainer, BuildTunnelParams}, 
    stack::{Stack, WeakStack}
};
use super::{
    container::*, 
    listener::*
};
use log::*;

const QUESTION_MAX_LEN: usize = 1024*25;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct RemoteSequence(DeviceId, TempSeq);

impl std::fmt::Display for RemoteSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{id:{}, seq:{:?}}}", self.0, self.1)
    }
}

impl From<(DeviceId, TempSeq)> for RemoteSequence {
    fn from(p: (DeviceId, TempSeq)) -> Self {
        Self(p.0, p.1)
    }
}

struct StreamEntries {
    id_entries: BTreeMap<IncreaseId, StreamContainer>,
    remote_entries: BTreeMap<RemoteSequence, StreamContainer>, 
}

impl StreamEntries {
    fn stream_of_id(&self, id: &IncreaseId) -> Option<StreamContainer> {
        self.id_entries.get(id).cloned()
    }

    fn stream_of_remote_sequence(&self, rs: &RemoteSequence) -> Option<StreamContainer> {
        self.remote_entries.get(rs).cloned()
    }

    fn remove_stream(&mut self, stream: &StreamContainer) -> (
        Option<IncreaseId>, 
        Option<RemoteSequence>
    ) {
        let remote_seq = RemoteSequence::from((stream.remote().0.clone(), stream.sequence()));
        (self.id_entries.remove(&stream.local_id()).map(|_| stream.local_id()), 
            self.remote_entries.remove(&remote_seq).map(|_| remote_seq))
    }
}

struct ReservingEntries {
    id_entries: LruCache<IncreaseId, StreamContainer>,
    remote_entries: LruCache<RemoteSequence, StreamContainer>, 
}

impl ReservingEntries {
    fn stream_of_id(&mut self, id: &IncreaseId) -> Option<StreamContainer> {
        self.id_entries.get(id).cloned()
    }

    fn stream_of_remote_sequence(&mut self, rs: &RemoteSequence) -> Option<StreamContainer> {
        self.remote_entries.get(rs).cloned()
    }
}

struct StreamManagerImpl {
    stack: WeakStack, 
    stream_entries: RwLock<StreamEntries>, 
    reserving_entries: Mutex<ReservingEntries>, 
    acceptor_entries: RwLock<BTreeMap<u16, StreamListener>>
}


#[derive(Clone)]
pub struct StreamManager(Arc<StreamManagerImpl>);

#[derive(Clone)]
pub struct WeakStreamManager(Weak<StreamManagerImpl>);

impl std::fmt::Display for StreamManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamManager {{local:{}}}", Stack::from(&self.0.stack).local_device_id())
    }
}

impl StreamManager {
    pub fn new(stack: WeakStack) -> Self {
        let strong_stack = Stack::from(&stack);
        Self(Arc::new(StreamManagerImpl {
            stack, 
            stream_entries: RwLock::new(StreamEntries {
                id_entries: BTreeMap::new(), 
                remote_entries: BTreeMap::new(), 
            }), 
            reserving_entries: Mutex::new(ReservingEntries {
                id_entries: LruCache::with_expiry_duration(strong_stack.config().stream.stream.package.msl), 
                remote_entries: LruCache::with_expiry_duration(strong_stack.config().stream.stream.package.msl), 
            }), 
            acceptor_entries: RwLock::new(BTreeMap::new())
        }))
    }

    fn to_weak(&self) -> WeakStreamManager {
        WeakStreamManager(Arc::downgrade(&self.0))
    }

    // connect完成是返回stream
    pub async fn connect(
        &self, 
        port: u16, 
        question: Vec<u8>, 
        build_params: BuildTunnelParams
    ) -> Result<StreamGuard, BuckyError> {
        if question.len() > QUESTION_MAX_LEN {
            return Err(BuckyError::new(
                BuckyErrorCode::Failed,
                format!("question's length large than {}", QUESTION_MAX_LEN),
            ));
        }

        info!("{} connect stream to {}:{} with params {}", self, build_params.remote_const.device_id(), port, build_params);
        let manager_impl = &self.0;
        let stack = Stack::from(&manager_impl.stack);
        let local_id = stack.id_generator().generate();
        let tunnel = stack.tunnel_manager().create_container(&build_params.remote_const)?;

        let stream = StreamContainer::new(
            manager_impl.stack.clone(), 
            tunnel.clone(), 
            port, 
            local_id, 
            tunnel.generate_sequence());
        manager_impl.stream_entries.write().unwrap().id_entries.insert(local_id, stream.clone());
        stream.connect(question, build_params).await.map_err(|err| {self.remove_stream(&stream, true); err})?;
        Ok(StreamGuard::from(stream))
    }

    pub fn listen(&self, port: u16) -> Result<StreamListenerGuard, BuckyError> {
        let stack = Stack::from(&self.0.stack);
        let mut entries = self.0.acceptor_entries.write().unwrap();
        match entries.get(&port) {
            Some(_) => {
                Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "port is listening"))
            },
            None => {
                let acceptor = StreamListener::new(self.to_weak(), port, stack.config().stream.listener.backlog);
                entries.insert(port, acceptor.clone());
                Ok(StreamListenerGuard::from(acceptor))
            }
        }.map(|v| {info!("{} listen on {}", self, port);v})
            .map_err(|err| {error!("{} listen on {} failed for {}", self, port, err); err})
    } 

    fn stream_of_id(&self, id: &IncreaseId) -> Option<StreamContainer> {
        self.0.stream_entries.read().unwrap().stream_of_id(id)
            .or_else(|| self.0.reserving_entries.lock().unwrap().stream_of_id(id))
    }

    pub(crate) fn stream_of_remote_sequence(&self, rs: &RemoteSequence) -> Option<StreamContainer> {
        self.0.stream_entries.read().unwrap().stream_of_remote_sequence(rs)
            .or_else(|| self.0.reserving_entries.lock().unwrap().stream_of_remote_sequence(rs))
    }

    pub(crate) fn remove_stream(&self, stream: &StreamContainer, reserving: bool) {
        info!("{} remove from stream manager", stream);
        let (local_id, remote_seq) = self.0.stream_entries.write().unwrap().remove_stream(stream);
        if reserving {
            info!("{} reserved closed in stream manager ", stream);
            let mut entries = self.0.reserving_entries.lock().unwrap();
            if let Some(local_id) = local_id {
                entries.id_entries.insert(local_id, stream.clone());
            }
            if let Some(remote_seq) = remote_seq {
                entries.remote_entries.insert(remote_seq, stream.clone());
            }            
        }
    }

    pub(crate) fn remove_acceptor(&self, acceptor: &StreamListener) {
        self.0.acceptor_entries.write().unwrap().remove(&acceptor.port());
    }

    fn try_accept(
        &self, 
        tunnel: TunnelGuard, 
        port: u16, 
        sequence: TempSeq, 
        remote_id: IncreaseId, 
        question: Vec<u8>) -> Option<StreamContainer> {
        match self.0.acceptor_entries.read().unwrap().get(&port).map(|a| a.clone()) {
            Some(acceptor) => {
                let manager_impl = &self.0;
                let local_id = Stack::from(&manager_impl.stack).id_generator().generate();
                let stream = StreamContainer::new(
                    manager_impl.stack.clone(), 
                    tunnel.clone(), 
                    port, 
                    local_id, 
                    sequence);
                stream.accept(remote_id);
                // 先加入到stream entries
                if let Some(exists) = {
                    let remote_seq = RemoteSequence(tunnel.remote().clone(), sequence);
                    let mut stream_entries = manager_impl.stream_entries.write().unwrap();
                    if let Some(exists) = stream_entries.remote_entries.get(&remote_seq) {
                        Some(exists.clone())
                    } else {
                        stream_entries.remote_entries.insert(remote_seq, stream.clone());
                        stream_entries.id_entries.insert(local_id, stream.clone());
                        None
                    }                    
                } {
                    let _ = stream.cancel_connecting_with(&BuckyError::new(BuckyErrorCode::AlreadyExists, "duplicate accepting stream"));
                    Some(exists)
                } else {
                    // 因为可能会失败，用guard保证reset掉，从stream entries中移除
                    let _ = acceptor.push_stream(PreAcceptedStream {
                        stream: StreamGuard::from(stream.clone()),
                        question
                    });
                    Some(stream)  
                }
            }, 
            None => {
                debug!("{} is not listening {}", self, port);
                None
            }
        }
    }

    pub(crate) fn on_statistic(&self) -> String {
        let stream_count = self.0.stream_entries.read().unwrap().id_entries.len();
        format!("StreamCount: {}", stream_count)
    }
}

impl From<&WeakStreamManager> for StreamManager {
    fn from(w: &WeakStreamManager) -> Self {
        Self(w.0.upgrade().unwrap())
    }
}

impl From<&WeakStreamManager> for Stack {
    fn from(w: &WeakStreamManager) -> Stack {
        Stack::from(&StreamManager::from(w).0.stack)
    }
}

impl OnPackage<SessionData, &TunnelContainer> for StreamManager {
    fn on_package(&self, pkg: &SessionData, tunnel: &TunnelContainer) -> Result<OnPackageResult, BuckyError> {
        let stack = Stack::from(&self.0.stack);
        match {
            if pkg.is_syn() {
                debug!("{} on {} from {}", self, pkg, tunnel.remote());
                let syn_info = pkg.syn_info.as_ref().unwrap();
                let remote_seq = RemoteSequence(tunnel.remote().clone(), syn_info.sequence);
                if let Some(stream) = self.stream_of_remote_sequence(&remote_seq) {
                    Some(stream)
                } else {
                    let mut question = vec![0; pkg.payload.as_ref().len()];
                    question.copy_from_slice(pkg.payload.as_ref());

                    self.try_accept(
                        stack.tunnel_manager().container_of(tunnel.remote()).unwrap(), 
                        syn_info.to_vport,
                        syn_info.sequence,  
                        pkg.session_id, 
                        question)
                }
            } else if pkg.is_syn_ack() {
                debug!("{} on {} from {}", self, pkg, tunnel.remote());
                let to_session_id = pkg.to_session_id.as_ref().unwrap();
                self.stream_of_id(to_session_id)
            } else {
                self.stream_of_id(&pkg.session_id)
            }
        } {
            Some(stream) => {
                stream.on_package(pkg, None)
            },
            None => {
                debug!("{} ingore {} for no valid stream", self, pkg);

                if !pkg.is_flags_contain(SESSIONDATA_FLAG_RESET) {
                    let mut rst_pkg = SessionData::new();
                    rst_pkg.flags_add(SESSIONDATA_FLAG_RESET);
                    rst_pkg.to_session_id = Some(pkg.session_id);
                    rst_pkg.send_time = bucky_time_now();

                    let _ = tunnel.send_package(DynamicPackage::from(rst_pkg), false);
                }

                Err(BuckyError::new(BuckyErrorCode::NotFound, "stream of id not found"))
            }
        }
    }
}


impl OnPackage<TcpSynConnection, (&TunnelContainer, tcp::AcceptInterface)> for StreamManager {
    fn on_package(&self, pkg: &TcpSynConnection, context: (&TunnelContainer, tcp::AcceptInterface)) -> Result<OnPackageResult, BuckyError> {
        let (tunnel, interface) = context;
        let remote_seq = RemoteSequence(tunnel.remote().clone(), pkg.sequence);
        let stack = Stack::from(&self.0.stack);
        match {
            if let Some(stream) = self.stream_of_remote_sequence(&remote_seq) {
                Some(stream)
            } else {
                let mut question = vec![0; pkg.payload.as_ref().len()];
                question.copy_from_slice(pkg.payload.as_ref());
                self.try_accept(
                    stack.tunnel_manager().container_of(tunnel.remote()).unwrap(), 
                    pkg.to_vport,
                    pkg.sequence,  
                    pkg.from_session_id, 
                    question)
            }
        } {
            Some(stream) => stream.on_package(pkg, interface), 
            None => Err(BuckyError::new(BuckyErrorCode::NotFound, "stream of id not found"))
        }
    }
}

// tcp 反连的请求
impl OnPackage<TcpSynConnection, &TunnelContainer> for StreamManager {
    fn on_package(&self, pkg: &TcpSynConnection, tunnel: &TunnelContainer) -> Result<OnPackageResult, BuckyError> {
        if pkg.reverse_endpoint.is_none() {
            return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "tcp syn connection should has reverse endpoints"));
        }
        let stack = Stack::from(&self.0.stack);
        let remote_seq = RemoteSequence(tunnel.remote().clone(), pkg.sequence);
        match {
            if let Some(stream) = self.stream_of_remote_sequence(&remote_seq) {
                Some(stream)
            } else {
                let mut question = vec![0; pkg.payload.as_ref().len()];
                question.copy_from_slice(pkg.payload.as_ref());
                if let Some(guard) = stack.tunnel_manager().container_of(tunnel.remote()) {
                    self.try_accept(
                        guard, 
                        pkg.to_vport,
                        pkg.sequence,  
                        pkg.from_session_id, 
                        question)
                } else {
                    error!("{} tunnel released, pkg={:?}, tunnel={}", self, pkg, tunnel);
                    None
                }
            }
        } {
            Some(stream) => stream.on_package(pkg, None), 
            None => Err(BuckyError::new(BuckyErrorCode::NotFound, "stream of id not found"))
        }
    }
}

impl OnPackage<TcpAckConnection, (&TunnelContainer, tcp::AcceptInterface)> for StreamManager {
    fn on_package(&self, pkg: &TcpAckConnection, context: (&TunnelContainer, tcp::AcceptInterface)) -> Result<OnPackageResult, BuckyError> {
        let (_tunnel, interface) = context;
        match self.stream_of_id(&pkg.to_session_id) {
            Some(stream) => stream.on_package(pkg, interface), 
            None => Err(BuckyError::new(BuckyErrorCode::NotFound, "stream of id not found"))
        }
    }
}


