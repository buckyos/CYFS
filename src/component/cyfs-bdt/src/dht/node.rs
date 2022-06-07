use super::k_bucket::*;
use crate::protocol::Datagram;
use async_std;
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_debug::Mutex;
use log::*;
use std::ops::Add;
use std::time::Duration;
use std::{
    collections::{BinaryHeap, HashSet},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc, Arc, RwLock,
    },
    task::{Context, Poll, Waker},
    time::SystemTime,
};

impl KadId for ObjectId {
    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp(other)
    }
    fn distance(&self, other: &Self) -> Self {
        let mut dist = ObjectId::default();
        let dist_bytes = dist.as_mut_slice();
        let self_bytes = self.as_slice();
        let other_bytes = other.as_slice();
        for i in 0usize..ObjectId::raw_bytes().unwrap() {
            dist_bytes[i] = self_bytes[i] ^ other_bytes[i];
        }

        dist
    }
    fn kad_index(dist: &Self) -> u32 {
        let self_bytes = dist.as_slice();
        for i in 0..ObjectId::raw_bytes().unwrap() {
            for j in 0..8 as u8 {
                if (self_bytes[i] >> (7 - j)) & 0x1 != 0 {
                    return (8 as usize * (ObjectId::raw_bytes().unwrap())) as u32
                        - (i * 8 + j as usize) as u32;
                }
            }
        }

        (ObjectId::raw_bytes().unwrap() * 8 - 1) as u32
    }
    fn bits() -> u32 {
        ObjectId::raw_bytes().unwrap() as u32
    }
}

//TODO 假定有一个这样的发送或者接收接口
#[async_trait]
pub trait TunnelInterface {
    fn send_package(&self, packages: Vec<Datagram>) -> Result<(), BuckyError>;
    async fn recv_tunnel_package_timeout(
        &self,
        vport: u16,
        timeout: u64,
    ) -> Result<Datagram, BuckyError>;
}

#[derive(Clone)]
struct NodeBucketEntry {
    desc: Device,
}
impl KadEntry for NodeBucketEntry {
    fn newest_than(&self, _other: &Self) -> bool {
        false
    }
}

pub type DhtValueType = SizedOwnedData<SizeU16>;
#[async_trait]
pub trait Dht: Send {
    async fn find_node(
        &mut self,
        id: &ObjectId,
        timeout: Duration,
    ) -> Result<Vec<(ObjectId, Device)>, BuckyError>;
    async fn find_value(&mut self, id: &ObjectId, timeout: Duration)
        -> Result<Vec<u8>, BuckyError>;
    async fn store(
        &mut self,
        id: &ObjectId,
        entry: &[u8],
        timeout: Duration,
    ) -> Result<(), BuckyError>;

    async fn store_obj(
        &mut self,
        obj: AnyNamedObject,
        timeout: Duration,
    ) -> Result<(), BuckyError> {
        let id = obj.calculate_id();
        let value = obj.encode_to_vec(true)?;
        self.store(&id, value.as_slice(), timeout).await
    }

    async fn find_obj(
        &mut self,
        id: &ObjectId,
        timeout: Duration,
    ) -> Result<AnyNamedObject, BuckyError> {
        let buf = self.find_value(id, timeout).await?;
        let (obj, _) = AnyNamedObject::raw_decode(buf.as_slice())?;
        Ok(obj)
    }
}

#[derive(Clone, PartialEq, Eq)]
enum FindType {
    Node(ObjectId),
    Value(ObjectId),
}

enum NodeRequest {
    Find(FindType),
    Store((ObjectId, DhtValueType)),
}

#[derive(Clone)]
enum FindResp {
    Node((ObjectId, Device)),
    Value(DhtValueType),
}

#[derive(Clone)]
enum NodeReply {
    Find(FindResp),
}

const FIND_STATE_NONE: u8 = 0;
const FIND_STATE_FIND: u8 = 1;
const FIND_STATE_FINISH: u8 = 2;
const FIND_STATE_TIMEOUT: u8 = 3;

struct PeerPair(ObjectId, Device);
impl PartialEq for PeerPair {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for PeerPair {}
impl Ord for PeerPair {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
impl PartialOrd for PeerPair {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

struct FindContext {
    out: Vec<FindResp>,
    querying: BinaryHeap<PeerPair>,
    queried: HashSet<ObjectId>,
    timeout: bool,
}

#[derive(Clone)]
struct FindSession {
    context: Arc<RwLock<FindContext>>,
    waker_list: Arc<Mutex<Vec<Waker>>>,
    state: Arc<AtomicU8>,
    find_type: FindType,
    sender: Arc<Mutex<Option<mpsc::Sender<FindResp>>>>,
}

struct FindFuture {
    waker_list: Arc<Mutex<Vec<Waker>>>,
    state: Arc<AtomicU8>,
}
impl Future for FindFuture {
    type Output = Result<(), BuckyError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut waker_list = self.waker_list.lock().unwrap();
        if self.state.load(Ordering::SeqCst) == FIND_STATE_FINISH {
            return Poll::Ready(Ok(()));
        }
        if self.state.load(Ordering::SeqCst) == FIND_STATE_TIMEOUT {
            return Poll::Ready(Err(BuckyError::from(BuckyErrorCode::Timeout)));
        }

        waker_list.push(cx.waker().clone());
        Poll::Pending
    }
}

impl FindSession {
    fn new(find_type: FindType) -> Self {
        Self {
            context: Arc::new(RwLock::new(FindContext {
                out: Vec::new(),
                querying: BinaryHeap::new(),
                queried: HashSet::new(),
                timeout: false,
            })),
            waker_list: Arc::new(Mutex::new(Vec::new())),
            state: Arc::new(AtomicU8::new(FIND_STATE_NONE)),
            find_type,
            sender: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Eq, PartialEq)]
enum InnerCmdType {
    FindNode = 6,
    FindValue = 7,
    Store = 8,
}

#[derive(Clone)]
pub struct BdtDhtNode<T: TunnelInterface + Send + Sync + Clone + 'static> {
    buckets: Arc<Mutex<KBuckets<ObjectId, NodeBucketEntry>>>,
    k_size: u32,
    sessions: Arc<RwLock<Vec<FindSession>>>,
    tunnel: Arc<T>,
    vport: u16,
    owner_id: Arc<ObjectId>,
    //owner_desc: Arc<Device>, //TODO 测试暂时先屏蔽
}

impl<T: TunnelInterface + Send + Sync + Clone + 'static> BdtDhtNode<T> {
    pub fn new(
        owner_id: ObjectId,
        /*owner_desc: Device,*/ tunnel: T,
        vport: u16,
        k_size: u32,
    ) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(KBuckets::<ObjectId, NodeBucketEntry>::new(
                k_size,
                owner_id.clone(),
            ))),
            k_size,
            sessions: Arc::new(RwLock::new(Vec::new())),
            tunnel: Arc::new(tunnel),
            vport,
            owner_id: Arc::new(owner_id),
            //owner_desc: Arc::new(owner_desc),
        }
    }

    pub fn init(&mut self) -> Result<(), BuckyError> {
        //TODO restore buckets from local
        self.package_loop();
        let mut dht_node = self.clone();
        async_std::task::spawn(async move {
            let owner = dht_node.owner_id.clone();
            let _ = dht_node
                .find_node(owner.as_ref(), Duration::from_secs(0))
                .await;
        });
        Ok(())
    }

    fn local_find_node(&mut self, id: &ObjectId) -> Vec<(ObjectId, Device)> {
        let buckets = self.buckets.lock().unwrap();
        let nearest = buckets.get_nearest_of(id);
        let mut nodes = Vec::new();
        for node in nearest {
            nodes.push((node.0.clone(), node.1.desc.clone()));
        }
        nodes
    }

    fn local_find_value(&mut self, _id: &ObjectId) -> Option<DhtValueType> {
        None
    }

    fn get_find_session(
        &mut self,
        find_type: &FindType,
        create_if_not_exist: bool,
    ) -> (Option<FindSession>, bool) {
        if create_if_not_exist {
            let mut sessions = self.sessions.write().unwrap();
            for session in sessions.iter() {
                if &session.find_type == find_type {
                    return (Some(session.clone()), false);
                }
            }
            let session = FindSession::new(find_type.clone());
            sessions.push(session.clone());
            return (Some(session), true);
        } else {
            let sessions = self.sessions.read().unwrap();
            for session in sessions.iter() {
                if &session.find_type == find_type {
                    return (Some(session.clone()), false);
                }
            }
            return (None, false);
        }
    }

    fn package_loop(&self) {
        let mut dht_node = self.clone();
        async_std::task::spawn(async move {
            loop {
                match dht_node
                    .tunnel
                    .recv_tunnel_package_timeout(dht_node.vport, 0 as u64)
                    .await
                {
                    Ok(p) if p.author_id.is_some() && p.author.is_some() => {
                        {
                            let mut buckets = dht_node.buckets.lock().unwrap();
                            buckets.set(
                                p.author_id.as_ref().unwrap().as_ref(),
                                &NodeBucketEntry {
                                    desc: p.author.as_ref().unwrap().clone(),
                                },
                            );
                        }
                        if p.inner_type as u8 > 0 {
                            //TODO 这里应该判断请求类型，先用inner_cmd_type，具体用哪个字段后面再说
                            match Self::request_from_package(&p) {
                                Err(e) => {
                                    warn!("parse request from package failed, e={}", e);
                                    return;
                                }
                                Ok(r) => {
                                    let reply = dht_node.handle_request(r);
                                    if let Some(mut items) = reply {
                                        while items.len() > 0 {
                                            let _ = dht_node.send_reply(&items.remove(0), &p);
                                        }
                                    }
                                }
                            }
                        } else {
                            match Self::reply_from_package(&p) {
                                Err(e) => {
                                    warn!("parse reply from package failed, e={}", e);
                                    return;
                                }
                                Ok(r) => {
                                    dht_node.handle_response(r.0, r.1);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn handle_response(&mut self, request: NodeRequest, reply: NodeReply) {
        match reply {
            NodeReply::Find(resp) => {
                if let NodeRequest::Find(find_type) = request {
                    let (session, _) = self.get_find_session(&find_type, false);
                    if session.is_none() {
                        return;
                    }

                    let session = session.unwrap();
                    let sender = session.sender.lock().unwrap().clone();
                    if sender.is_none() {
                        return;
                    }
                    let _ = sender.unwrap().send(resp);
                }
            }
        }
    }

    //find node的时候一个mtu发送不了所有的desc，所以一个一个回复，所以这里应该是N个Reply
    fn handle_request(&mut self, request: NodeRequest) -> Option<Vec<NodeReply>> {
        match request {
            NodeRequest::Find(find_type) => match find_type {
                FindType::Node(id) => {
                    let mut local = self.local_find_node(&id);
                    if local.len() == 0 {
                        return None;
                    }
                    let mut reply = Vec::new();
                    while local.len() > 0 {
                        reply.push(NodeReply::Find(FindResp::Node(local.remove(0))));
                    }

                    Some(reply)
                }
                FindType::Value(id) => {
                    let value = self.local_find_value(&id);
                    if value.is_some() {
                        return Some(vec![NodeReply::Find(FindResp::Value(value.unwrap()))]);
                    }

                    let mut local = self.local_find_node(&id);
                    if local.len() == 0 {
                        return None;
                    }
                    let mut reply = Vec::new();
                    while local.len() > 0 {
                        reply.push(NodeReply::Find(FindResp::Node(local.remove(0))));
                    }

                    Some(reply)
                }
            },
            NodeRequest::Store(_info) => None,
        }
    }

    fn run_session_find(
        &self,
        session: &mut FindSession,
        remotes: &mut Vec<(ObjectId, Device)>,
        timeout: Duration,
    ) -> impl Future<Output = Result<(), BuckyError>> {
        if session
            .state
            .compare_exchange(
                FIND_STATE_NONE,
                FIND_STATE_FIND,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            if remotes.len() > 0 {
                while remotes.len() > 0 {
                    let mut context = session.context.write().unwrap();
                    let peer = remotes.remove(0);
                    if context.queried.insert(peer.0.clone()) {
                        context
                            .querying
                            .push(PeerPair(peer.0.clone(), peer.1.clone()));
                        context.out.push(FindResp::Node(peer));
                    }
                }
            }

            let find_context = session.context.clone();
            let state = session.state.clone();
            let waker_list = session.waker_list.clone();
            let find_type = session.find_type.clone();
            let (sender, receiver) = mpsc::channel::<FindResp>();
            {
                let mut self_sender = session.sender.lock().unwrap();
                *self_sender = Some(sender);
            }

            let end_time = SystemTime::now().add(timeout);

            let bdt_node = self.clone();
            async_std::task::spawn(async move {
                let req = NodeRequest::Find(find_type);
                loop {
                    let context = find_context.clone();
                    let node = {
                        let mut context = context.write().unwrap();
                        context.querying.pop()
                    };
                    if node.is_none() {
                        if state
                            .compare_exchange(
                                FIND_STATE_FIND,
                                FIND_STATE_FINISH,
                                Ordering::SeqCst,
                                Ordering::SeqCst,
                            )
                            .is_ok()
                        {
                            Self::wake_find_future(waker_list);
                        }
                        break;
                    }
                    let node = node.unwrap();

                    let _ = bdt_node.send_req(&req, node.0, node.1);

                    match receiver.recv_timeout(timeout) {
                        Ok(resp) => match resp {
                            FindResp::Node(node) => {
                                let mut context = context.write().unwrap();
                                if context.queried.insert(node.0.clone()) {
                                    context
                                        .querying
                                        .push(PeerPair(node.0.clone(), node.1.clone()));
                                    context.out.push(FindResp::Node(node));
                                }
                            }
                            FindResp::Value(v) => {
                                let mut context = context.write().unwrap();
                                context.out.clear();
                                context.out.push(FindResp::Value(v));
                                break;
                            }
                        },
                        _ => {
                            if SystemTime::now() > end_time {
                                if state
                                    .compare_exchange(
                                        FIND_STATE_FIND,
                                        FIND_STATE_TIMEOUT,
                                        Ordering::SeqCst,
                                        Ordering::SeqCst,
                                    )
                                    .is_ok()
                                {
                                    Self::wake_find_future(waker_list.clone());
                                }
                            }
                        }
                    }
                }
            });
        }

        FindFuture {
            waker_list: session.waker_list.clone(),
            state: session.state.clone(),
        }
    }

    fn wake_find_future(waker_list: Arc<Mutex<Vec<Waker>>>) {
        let mut waker_list = {
            let mut out_list = Vec::new();
            let mut list = waker_list.lock().unwrap();
            out_list.append(&mut list);
            out_list
        };
        while waker_list.len() > 0 {
            let waker = waker_list.remove(0);
            waker.wake();
        }
    }

    fn request_from_package(_: &Datagram) -> Result<NodeRequest, BuckyError> {
        Err(BuckyError::from(BuckyErrorCode::Timeout))
    }

    fn reply_from_package(_: &Datagram) -> Result<(NodeRequest, NodeReply), BuckyError> {
        Err(BuckyError::from(BuckyErrorCode::Timeout))
    }

    fn send_req(
        &self,
        req: &NodeRequest,
        _remote_id: ObjectId,
        _remote_desc: Device,
    ) -> Result<(), BuckyError> {
        match req {
            NodeRequest::Find(find_type) => {
                let mut buff = [0 as u8; 32];
                match find_type {
                    FindType::Node(id) => {
                        let _ = id.raw_encode(buff.as_mut(), &None)?;
                    }
                    FindType::Value(key) => {
                        let _ = key.raw_encode(buff.as_mut(), &None)?;
                    }
                }
                //let _data = SizedOwnedData(buff.to_vec());
            }
            NodeRequest::Store(kv) => {
                let mut buff = [0 as u8; 2048];
                let b = kv.0.raw_encode(buff.as_mut(), &None)?;
                let _b = kv.1.raw_encode(b, &None)?;
                //let _data = SizedOwnedData(buff.to_vec());
            }
        }
        Ok(())
    }

    fn send_reply(&self, reply: &NodeReply, _from: &Datagram) -> Result<(), BuckyError> {
        match reply {
            NodeReply::Find(find_resp) => {
                let mut buff = [0 as u8; 2048];
                match find_resp {
                    FindResp::Value(_v) => {
                        //TODO
                        //let _b = kv.0.raw_encode(buff.as_mut())?;
                    }
                    FindResp::Node(n) => {
                        let b = n.0.raw_encode(buff.as_mut(), &None)?;
                        let _b = n.1.raw_encode(b, &None)?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl<T: TunnelInterface + Send + Sync + Clone + 'static> Dht for BdtDhtNode<T> {
    async fn find_node(
        &mut self,
        id: &ObjectId,
        timeout: Duration,
    ) -> Result<Vec<(ObjectId, Device)>, BuckyError> {
        let mut local = self.local_find_node(id);
        for info in local.iter() {
            if &info.0 == id {
                return Ok(vec![(id.clone(), info.1.clone())]);
            }
        }

        let find_type = FindType::Node(id.clone());
        let (session, _new_create) = self.get_find_session(&find_type, true);
        assert!(session.is_some());
        let mut session = session.unwrap();
        self.run_session_find(&mut session, &mut local, timeout)
            .await?;
        let context = session.context.read().unwrap();
        let mut nearest = Vec::new();
        for item in context.out.iter() {
            match item {
                FindResp::Node(n) => {
                    nearest.push(n.clone());
                }
                _ => {
                    assert!(false);
                }
            }
        }
        nearest.sort_by(|a, b| a.0.distance(id).compare(&b.0.distance(id)));
        nearest.truncate(self.k_size as usize);
        Ok(nearest)
    }

    async fn find_value(
        &mut self,
        id: &ObjectId,
        timeout: Duration,
    ) -> Result<Vec<u8>, BuckyError> {
        let local = self.local_find_value(id);
        if let Some(v) = local {
            return Ok(v.take());
        }

        let find_type = FindType::Value(id.clone());
        let (session, new_create) = self.get_find_session(&find_type, true);
        assert!(session.is_some());
        let mut session = session.unwrap();
        if new_create {
            let mut local = self.local_find_node(id);
            self.run_session_find(&mut session, &mut local, timeout)
                .await?;
        } else {
            self.run_session_find(&mut session, &mut Vec::new(), timeout)
                .await?;
        }

        let context = session.context.read().unwrap();
        if context.out.len() > 0 {
            match context.out[0].clone() {
                FindResp::Value(v) => Ok(v.take()),
                _ => {
                    assert!(false);
                    Err(BuckyError::from(BuckyErrorCode::NotFound))
                }
            }
        } else {
            Err(BuckyError::from(BuckyErrorCode::NotFound))
        }
    }

    async fn store(
        &mut self,
        id: &ObjectId,
        value: &[u8],
        timeout: Duration,
    ) -> Result<(), BuckyError> {
        let mut nearest = self.find_node(id, timeout).await?;
        let req = NodeRequest::Store((id.clone(), DhtValueType::from(Vec::from(value)))); // FIXME 去copy
        while nearest.len() > 0 {
            let peer = nearest.remove(0);
            self.send_req(&req, peer.0, peer.1)?
        }
        Ok(())
    }
}
