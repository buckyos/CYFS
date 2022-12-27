use std::{
    sync::{Arc, Weak, RwLock}, 
    collections::{BTreeMap, LinkedList}, 
    time::{Duration}, 
};
use async_std::task;
use futures::future::AbortRegistration;
use cyfs_base::*;
use crate::{
    types::*,
    interface::{udp::{Interface}}, 
    protocol::{*, v0::*}, 
    history::keystore, 
    stack::{WeakStack, Stack}
};
use super::{
    udp::{self, *}, 
    tcp::{*}
};


#[derive(Clone)]
pub struct CallConfig {
    pub timeout: Duration, 
    pub first_try_timeout: Duration, 
    pub udp: udp::Config, 
}


struct ManagerImpl {
    stack: WeakStack,
    seq_genarator: TempSeqGenerator,
    sessions: RwLock<BTreeMap<TempSeq, WeakSessions>>,
}

#[derive(Clone)]
pub struct CallManager(Arc<ManagerImpl>);

impl CallManager {
    pub fn create(stack: WeakStack) -> Self {
        Self(Arc::new(ManagerImpl {
            stack,
            seq_genarator: TempSeqGenerator::new(),
            sessions: RwLock::new(BTreeMap::new()),
        }))
    }

    pub async fn call(
        &self, 
        reverse_endpoints: Option<&[Endpoint]>, 
        remote: &DeviceId, 
        sn_list: &Vec<DeviceId>, 
        payload_generater: impl Fn(&SnCall) -> Vec<u8>
    ) -> BuckyResult<CallSessions> {
        let seq = self.0.seq_genarator.generate();
    
        let stack = Stack::from(&self.0.stack);
        let active_pn_list = stack.proxy_manager().active_proxies();
        let local_device = stack.sn_client().ping().default_local();

        let mut sessions = vec![];
        for sn_id in sn_list {
            let sn = stack.device_cache().get_inner(sn_id).ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "sn device not cached"))?;
            let mut call = SnCall {
                protocol_version: 0, 
                stack_version: 0, 
                seq: seq,
                to_peer_id: remote.clone(),
                from_peer_id: stack.local_device_id().clone(),
                sn_peer_id: sn_id.clone(),
                reverse_endpoint_array: reverse_endpoints.map(|ep_list| Vec::from(ep_list)),
                active_pn_list: if active_pn_list.len() > 0 {
                    Some(active_pn_list.clone())
                } else {
                    None
                }, 
                peer_info: Some(local_device.clone()),
                payload: SizedOwnedData::from(vec![]),
                send_time: 0,
                is_always_call: false
            };
            call.payload = SizedOwnedData::from(payload_generater(&call));
            let session = CallSession::new(self.0.stack.clone(), call, stack.config().sn_client.call.clone()).await;
            let net_listener = stack.net_manager().listener();

            let mut cached = false;
            if let Some(active) = stack.sn_client().cache().get_active(sn_id) {
                if sn.connect_info().endpoints().iter().find(|ep| active.remote().eq(ep)).is_some() {
                    if active.is_udp() {
                        if let Some(local) = net_listener.udp_of(active.local()) {
                            let tunnel = UdpCall::new(session.to_weak(), vec![local.clone()], vec![active.remote().clone()]);
                            session.add_tunnel(tunnel.clone_as_call_tunnel());
                            cached = true;
                        }
                    } else {
                        if let Some(local) = net_listener.tcp_of(active.local()) {
                            let tunnel = TcpCall::new(session.to_weak(), stack.config().sn_client.call.timeout, active.remote().clone());
                            session.add_tunnel(tunnel.clone_as_call_tunnel());
                            cached = true;
                        }
                    }
                }
            }
            
            if !cached {
                stack.sn_client().cache().remove_active(sn_id);
                {
                    let locals = net_listener.udp().iter().filter(|interface| interface.local().addr().is_ipv4()).cloned().collect();
                    let remotes = sn.connect_info().endpoints().iter().filter(|endpoint| endpoint.is_udp() && endpoint.addr().is_ipv4()).cloned().collect();
                    let tunnel = UdpCall::new(session.to_weak(), locals, remotes);
                    session.add_tunnel(tunnel.clone_as_call_tunnel());
                }
            
                if net_listener.tcp().iter().find(|l| l.local().addr().is_ipv4()).is_some() {
                    for remote in  sn.connect_info().endpoints().iter().filter(|endpoint| endpoint.is_tcp() && endpoint.addr().is_ipv4()) {
                        let tunnel = TcpCall::new(session.to_weak(), stack.config().sn_client.call.timeout, remote.clone());
                        session.add_tunnel(tunnel.clone_as_call_tunnel());
                    }
                }
            }

            sessions.push(session);
        }   
        
        let sessions = CallSessions::new(seq, remote.clone(), sessions);
        self.0.sessions.write().unwrap().insert(seq, sessions.to_weak());

        Ok(sessions)
    }

    pub(crate) fn on_time_escape(&self, now: Timestamp) {
        let mut alive = LinkedList::new();

        {
            let mut dead = LinkedList::new();
            let mut sessions = self.0.sessions.write().unwrap();
            for (seq, weak) in &*sessions {
                if let Some(session) = weak.to_strong() {
                    alive.push_back(session);
                } else {
                    dead.push_back(seq.clone())
                }
            }

            for seq in dead {
                sessions.remove(&seq);
            }
        }
        
        for session in alive {
            session.on_time_escape(now);
        }
    }


    pub(crate) fn on_udp_call_resp(&self, resp: &SnCallResp, local: &Interface, from: &Endpoint) {
        let session = self.0.sessions.read().unwrap().get(&resp.seq).cloned().and_then(|weak| weak.to_strong());
        if let Some(session) = session {
            session.on_udp_call_resp(resp, local, from);
        }
    }
}


#[derive(Clone)]
pub struct CallSessions(Arc<SessionsImpl>);

#[derive(Clone)]
struct WeakSessions(Weak<SessionsImpl>);

impl WeakSessions {
    fn to_strong(&self) -> Option<CallSessions> {
        self.0.upgrade().map(|ptr| CallSessions(ptr))
    }
}

enum SessionsState {
    Init(StateWaiter), 
    Running {
        waiter: StateWaiter, 
        pending: LinkedList<CallSession>, 
        finished: LinkedList<CallSession>, 
    },
    Finished, 
    Canceled(BuckyError)
}

struct SessionsImpl {
    seq: TempSeq, 
    remote: DeviceId, 
    sessions: Vec<CallSession>, 
    state: RwLock<SessionsState>
}

impl CallSessions {
    fn to_weak(&self) -> WeakSessions {
        WeakSessions(Arc::downgrade(&self.0))
    }

    fn new(seq: TempSeq, remote: DeviceId, sessions: Vec<CallSession>) -> Self {
        Self(Arc::new(SessionsImpl {
            seq, 
            remote, 
            sessions, 
            state: RwLock::new(SessionsState::Init(StateWaiter::new()))
        }))
    }

    fn sync_session(&self, session: CallSession) {
        let waiter = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                SessionsState::Running {
                    waiter, 
                    finished, 
                    pending,
                } => {
                    finished.push_back(session.clone());
                    pending.push_back(session);
                    waiter.pop()
                },  
                SessionsState::Canceled(_) => None, 
                _ => unreachable!()
            }
        };

        if let Some(waiter) = waiter {
            waiter.abort();
        }
    }

    fn on_udp_call_resp(&self, resp: &SnCallResp, local: &Interface, from: &Endpoint) {
        if let Some(session) = self.0.sessions.iter().find(|session| resp.sn_peer_id.eq(session.sn())) {
            session.on_udp_call_resp(resp, local, from);
        }
    }

    pub async fn next(&self) -> BuckyResult<Option<CallSession>> {
        enum NextStep {
            Start(AbortRegistration), 
            Return(BuckyResult<Option<CallSession>>), 
            Wait(AbortRegistration)
        }

        let next = {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                SessionsState::Init(waiter) => {
                    assert_eq!(waiter.len(), 0);
                    let next = NextStep::Start(waiter.new_waiter());
                    *state = SessionsState::Running {
                        waiter: waiter.transfer(), 
                        pending: LinkedList::new(), 
                        finished: LinkedList::new()
                    };
                    next
                }, 
                SessionsState::Running {
                    waiter,  
                    pending, 
                    ..
                } => {
                    assert_eq!(waiter.len(), 0);
                    if pending.len() > 0 {
                        NextStep::Return(Ok(pending.pop_front()))
                    } else {
                        NextStep::Wait(waiter.new_waiter())
                    }   
                },  
                SessionsState::Finished => NextStep::Return(Ok(None)), 
                SessionsState::Canceled(err) => NextStep::Return(Err(err.clone()))
            }
        };

        let state = || {
            let mut state = self.0.state.write().unwrap();
            match &mut *state {
                SessionsState::Running { 
                    pending, 
                    finished, 
                    ..
                } => {
                    let ret = Ok(pending.pop_front());
                    if pending.len() == 0 && finished.len() == self.0.sessions.len() {
                        *state = SessionsState::Finished;
                    }
                    ret
                },  
                SessionsState::Finished => Ok(None), 
                SessionsState::Canceled(err) => Err(err.clone()),
                _ => unreachable!()
            }
        };
        
        match next {
            NextStep::Start(waiter) => {
                for session in &self.0.sessions {
                    let sessions = self.clone();
                    let session = session.clone();
                    task::spawn(async move {
                        let _ = session.wait().await;
                        sessions.sync_session(session);
                    });
                }
                StateWaiter::wait(waiter, state).await
            }, 
            NextStep::Wait(waiter) => {
                StateWaiter::wait(waiter, state).await
            },
            NextStep::Return(result) => result
        }
    }

    fn on_time_escape(&self, now: Timestamp) {
        for session in &self.0.sessions {
            session.on_time_escape(now);
        }
    }
}

#[async_trait::async_trait]
pub(super) trait CallTunnel: Send + Sync {
    fn clone_as_call_tunnel(&self) -> Box<dyn CallTunnel>;
    async fn wait(&self) -> (BuckyResult<Device>, Option<EndpointPair>);
    fn cancel(&self);
    fn on_time_escape(&self, _now: Timestamp) {

    }
    fn reset(&self, _timeout: Duration) -> Option<Box<dyn CallTunnel>> {
        None
    }
    fn on_udp_call_resp(&self, _resp: &SnCallResp, _local: &Interface, _from: &Endpoint) {

    }
}

enum SessionState {
    Init, 
    FirstTry, 
    SecondTry, 
    Responsed {
        active: EndpointPair, 
        result: BuckyResult<Device>
    }, 
    Canceled(BuckyError)
}

struct SessionStateImpl {
    packages: Arc<PackageBox>, 
    tunnels: Vec<Box<dyn CallTunnel>>, 
    waiter: StateWaiter, 
    start_at: Timestamp, 
    state: SessionState
}

struct SessionImpl {
    stack: WeakStack, 
    sn: DeviceId, 
    config: CallConfig, 
    state: RwLock<SessionStateImpl>
}


#[derive(Clone)]
pub struct CallSession(Arc<SessionImpl>);

#[derive(Clone)]
pub(super) struct WeakSession(Weak<SessionImpl>);

impl WeakSession {
    pub fn to_strong(&self) -> Option<CallSession> {
        self.0.upgrade().map(|ptr| CallSession(ptr))
    }
}

impl CallSession {
    pub(super) fn to_weak(&self) -> WeakSession {
        WeakSession(Arc::downgrade(&self.0))
    }

    async fn new(stack: WeakStack, call: SnCall, config: CallConfig) -> Self {
        let strong_stack = Stack::from(&stack);
        let sn = call.sn_peer_id.clone();
        let key_stub = strong_stack.keystore().create_key(strong_stack.device_cache().get_inner(&sn).unwrap().desc(), true);
        let mut packages = PackageBox::encrypt_box(sn.clone(), key_stub.key.clone());
        if let keystore::EncryptedKey::Unconfirmed(encrypted) = &key_stub.encrypted {
            let mut exchange = Exchange::from((&call, encrypted.clone(), key_stub.key.mix_key.clone()));
            let _ = exchange.sign(strong_stack.keystore().signer()).await.unwrap();
            packages.push(exchange);
        }
        packages.push(call);

        Self(Arc::new(SessionImpl {
            stack, 
            sn, 
            config, 
            state: RwLock::new(SessionStateImpl {
                packages: Arc::new(packages), 
                tunnels: vec![], 
                waiter: StateWaiter::new(),
                start_at: 0,  
                state: SessionState::Init, 
            })
        }))
    }

    fn sn(&self) -> &DeviceId {
        &self.0.sn
    }

    pub(super) fn stack(&self) -> Stack {
        Stack::from(&self.0.stack)
    }

    fn on_udp_call_resp(&self, resp: &SnCallResp, local: &Interface, from: &Endpoint) {
        let tunnels = {
            let state = self.0.state.read().unwrap();
            match &state.state {
                SessionState::FirstTry | SessionState::SecondTry => state.tunnels.iter().map(|t| t.clone_as_call_tunnel()).collect(), 
                _ => vec![]
            }
        };

        for tunnel in tunnels {
            tunnel.on_udp_call_resp(resp, local, from);
        }
    }


    fn sync_tunnel(&self, _tunnel: &dyn CallTunnel, result: BuckyResult<Device>, active: Option<EndpointPair>) {
        struct NextStep {
            waiter: Option<StateWaiter>, 
            to_cancel: Vec<Box<dyn CallTunnel>>, 
            update_cache: Option<EndpointPair>
        }

        impl NextStep {
            fn none() -> Self {
                Self {
                    waiter: None,
                    to_cancel: vec![], 
                    update_cache: None
                }
            }
        }


        let next = {
            let mut next = NextStep::none();
            let mut state = self.0.state.write().unwrap();

            match &state.state {
                SessionState::FirstTry | SessionState::SecondTry => {
                    if let Some(active) = active {
                        next.update_cache = Some(active.clone());
                        state.state = SessionState::Responsed { active, result };
                        next.waiter = Some(state.waiter.transfer());
                    }
                }, 
                _ => {}
            };

            if next.waiter.is_some() {
                std::mem::swap(&mut next.to_cancel, &mut state.tunnels);
            }

            next
        };

        if let Some(endpoint) = next.update_cache {
            let stack = Stack::from(&self.0.stack);
            stack.sn_client().cache().add_active(self.sn(), endpoint);
        }
       
        if let Some(waiter) = next.waiter {
            waiter.wake();
        }

        for tunnel in next.to_cancel {
            tunnel.cancel();
        }
    }

    pub fn config(&self) -> &CallConfig {
        &self.0.config
    }

    async fn wait(&self) -> Option<EndpointPair> {
        enum NextStep {
            Start(AbortRegistration, Vec<Box<dyn CallTunnel>>), 
            Wait(AbortRegistration),
            Return(Option<EndpointPair>)
        }

        let next = {
            let mut state = self.0.state.write().unwrap();

            match &state.state {
                SessionState::Init => {
                    if state.packages.has_exchange() {
                        state.state = SessionState::SecondTry;
                    } else {
                        state.state = SessionState::FirstTry;
                    }
                    state.start_at = bucky_time_now();
                    NextStep::Start(state.waiter.new_waiter(), state.tunnels.iter().map(|t| t.clone_as_call_tunnel()).collect())
                }, 
                SessionState::Responsed { active, .. } => NextStep::Return(Some(active.clone())), 
                SessionState::Canceled(_) => NextStep::Return(None), 
                _ => NextStep::Wait(state.waiter.new_waiter())
            }
        };

        let state = || {
            let state = self.0.state.read().unwrap();
            match &state.state {
                SessionState::Responsed { active, .. } => Some(active.clone()), 
                SessionState::Canceled(_) => None, 
                _ => unreachable!()
            }
        };
        
        match next {
            NextStep::Start(waiter, tunnels) => {
                for tunnel in tunnels {
                    let session = self.clone();
                    task::spawn(async move {
                        let (result, active) = tunnel.wait().await;
                        session.sync_tunnel(tunnel.as_ref(), result, active);
                    });
                }
                StateWaiter::wait(waiter, state).await
            }, 
            NextStep::Wait(waiter) => StateWaiter::wait(waiter, state).await,
            NextStep::Return(active) => active
        }
    }

    fn add_tunnel(&self, tunnel: Box<dyn CallTunnel>) {
        let mut state = self.0.state.write().unwrap();
        match &state.state {
            SessionState::Init => {
                state.tunnels.push(tunnel);
            },
            _ => unreachable!()
        }
    }

    pub fn packages(&self) -> Arc<PackageBox> {
        self.0.state.read().unwrap().packages.clone()
    }


    async fn reset(&self, call: SnCall) {
        let stack = Stack::from(&self.0.stack);
        stack.keystore().reset_peer(self.sn());

        let key_stub = stack.keystore().create_key(stack.device_cache().get_inner(self.sn()).unwrap().desc(), true);
        let mut packages = PackageBox::encrypt_box(self.sn().clone(), key_stub.key.clone());
        if let keystore::EncryptedKey::Unconfirmed(encrypted) = &key_stub.encrypted {
            let mut exchange = Exchange::from((&call, encrypted.clone(), key_stub.key.mix_key.clone()));
            let _ = exchange.sign(stack.keystore().signer()).await.unwrap();
            packages.push(exchange);
        }
        packages.push(call);

        let tunnels = {
            let mut state = self.0.state.write().unwrap();
            state.packages = Arc::new(packages);
            let escaped = Duration::from_micros(bucky_time_now() - state.start_at);
            if escaped < self.config().timeout {
                let remain = self.config().timeout - escaped;
                let mut resets = vec![];
                for tunnel in &state.tunnels {
                    if let Some(reset) = tunnel.reset(remain) {
                        resets.push(reset);
                    }
                }
                
                for tunnel in &resets {
                    state.tunnels.push(tunnel.clone_as_call_tunnel());
                }
               
                Some(resets)
            } else {
                None
            }
        };
       
        if let Some(tunnels) = tunnels {
            for tunnel in tunnels {
                let session = self.clone();
                task::spawn(async move {
                    let (result, active) = tunnel.wait().await;
                    session.sync_tunnel(tunnel.as_ref(), result, active);
                });
            }
        }
    }

    fn on_time_escape(&self, now: Timestamp) {
        struct NextStep {
            waiter: Option<StateWaiter>, 
            reset: Option<SnCall>, 
            callback: Option<Vec<Box<dyn CallTunnel>>>, 
        }

        impl NextStep {
            fn none() -> Self {
                Self {
                    waiter: None, 
                    reset: None, 
                    callback: None
                }
            }
        }

        let mut state = self.0.state.write().unwrap();
        let mut next = NextStep::none();
        match &state.state {
            SessionState::FirstTry => {
                if now > state.start_at && Duration::from_micros(now - state.start_at) > self.config().first_try_timeout {
                    let call: &SnCall = state.packages.packages_no_exchange()[0].as_ref();
                    next.reset = Some(call.clone());
                    state.state = SessionState::SecondTry;
                } else {
                    next.callback = Some(state.tunnels.iter().map(|t| t.clone_as_call_tunnel()).collect());
                }
            }, 
            SessionState::SecondTry => {
                if now > state.start_at && Duration::from_micros(now - state.start_at) > self.config().timeout {
                    state.state = SessionState::Canceled(BuckyError::new(BuckyErrorCode::Timeout, "session timeout"));
                    next.waiter = Some(state.waiter.transfer());
                } else {
                    next.callback = Some(state.tunnels.iter().map(|t| t.clone_as_call_tunnel()).collect());
                }
            }, 
            _ => {}
        }

        if let Some(waiter) = next.waiter {
            waiter.wake();
        }

        if let Some(tunnels) = next.callback {
            for tunnel in tunnels {
                tunnel.on_time_escape(now);
            }
        }

        if let Some(call) = next.reset {
            let session = self.clone();
            task::spawn(async move {
                session.reset(call).await;
            });
        }
    }

    pub fn result(&self) -> Option<BuckyResult<Device>> {
        let state = self.0.state.read().unwrap();
        match &state.state {
            SessionState::Responsed { result, .. } => Some(result.clone()), 
            SessionState::Canceled(err) => Some(Err(err.clone())), 
            _ => None
        }
    }
}


