use std::{
    sync::{Arc, RwLock, atomic::{self, AtomicU8, AtomicU32, AtomicU64, AtomicBool}}, 
    collections::{HashMap, hash_map::Entry},
    convert::TryFrom, 
    time::{SystemTime, Instant, Duration, UNIX_EPOCH}
};
use async_std::{
    task
};

use cyfs_base::*;
use cyfs_debug::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::{NetListener, UpdateOuterResult, udp::{Interface, PackageBoxEncodeContext}}, 
    history::keystore, 
    stack::{WeakStack, Stack} 
};
use super::super::types::*;
use super::{ 
    Config,
};


const INVALID_CALL_DELAY: u16 = 0xFFFF;
const LOCAL_IPV6_RETAIN: bool = false;

pub trait ServiceAppraiser: Send + Sync {
    // 对SN服务进行评分，可以依据本地记录的服务清单和SN提供的服务清单作对比进行评价；
    // 因为客户端向SN提供的服务清单可能丢失，所以还要参照上次提供给SN的服务清单：
    // local_receipt：从上次向SN提供服务清单后产生的服务清单
    // last_receipt: 上次向SN提供的可能丢失的服务清单
    fn appraise(
        &self, 
        sn: &Device, 
        local_receipt: &Option<SnServiceReceipt>, 
        last_receipt: &Option<SnServiceReceipt>, 
        receipt_from_sn: &Option<SnServiceReceipt>
    ) -> SnServiceGrade;
}

pub trait PingClientStateEvent: Send + Sync {
    fn online(&self, sn: &Device);
    fn offline(&self, sn: &Device);
}

pub trait PingClientCalledEvent<Context=()>: Send + Sync {
    fn on_called(&self, called: &SnCalled, context: Context) -> Result<(), BuckyError>;
}


struct PingEnv {
    stack: WeakStack, 
    ping_interval_init_ms: u32,
    ping_interval_ms: u32,
    offline_ms: u32,
}

pub(crate) struct PingManager {
    env: Arc<PingEnv>,
    clients: RwLock<HashMap<DeviceId, Arc<Client>>>,
    is_started: AtomicBool, // 暂时没有多次重启需求，简单做个标记
}

impl std::fmt::Display for PingManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stack = Stack::from(&self.env.stack);
        write!(f, "PingManager{{local:{}}}", stack.local_device_id())
    }
}

impl PingManager {
    pub fn create(stack: WeakStack, config: &Config) -> PingManager {
        PingManager {
            env: Arc::new(PingEnv {
                stack,
                ping_interval_init_ms: config.ping_interval_init.as_millis() as u32,
                ping_interval_ms: config.ping_interval.as_millis() as u32,
                offline_ms: config.offline.as_millis() as u32
            }),
            clients: RwLock::new(Default::default()),
            is_started: AtomicBool::new(false)
        }
    }

    pub fn sn_list(&self) -> Vec<DeviceId> {
        self.clients.read().unwrap().iter().map(|(sn, _)| sn.clone()).collect()
    }

    pub fn status_of(&self, sn: &DeviceId) -> Option<SnStatus> {
        self.clients.read().unwrap().get(sn).map(|client| client.status())
    }

    // 暂时没有重复启停的需求
    pub fn start(&self) {
        log::info!("{} starting.", self);

        self.is_started.store(true, atomic::Ordering::Release);
        let clients: Vec<Arc<Client>> = self.clients.read().unwrap().iter().map(|c| c.1.clone()).collect();
        for client in clients {
            client.start();
        }

        log::info!("{} started.", self);
    }

    pub fn stop(&self) -> Result<(), BuckyError> {
        log::info!("{} stopping.", self);
        self.is_started.store(false, atomic::Ordering::Release);
        let clients: Vec<Arc<Client>> = self.clients.read().unwrap().iter().map(|c| c.1.clone()).collect();
        for client in clients {
            client.stop();
        }

        log::info!("{} stopped.", self);
        Ok(())
    }

    pub fn add_sn(&self, desc: &Device, is_encrypto: bool, appraiser: Box<dyn ServiceAppraiser>) -> Result<(), BuckyError> {
        let peerid = desc.desc().device_id();

        log::info!("{} add-sn: {}", self, peerid.to_string());

        let _ = self.remove_sn(&peerid);

        // <TODO>加载历史数据

        let client = Arc::new(Client::new(self, desc, is_encrypto, appraiser));

        self.clients.write().unwrap().insert(peerid.clone(), client.clone());

        if self.is_started.load(atomic::Ordering::Acquire) {
            client.start();
        }

        Ok(())
    }

    pub fn remove_sn(&self, peerid: &DeviceId) -> Result<(), BuckyError> {
        let client = self.clients.write().unwrap().remove(peerid);
        match client {
            None => Err(BuckyError::new(BuckyErrorCode::NotFound, "not found the sn")),
            Some(client) => {
                // <TODO>保存服务清单
                client.stop();
                Ok(())
            }
        }
    }

    pub fn on_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface) -> Result<(), BuckyError> {
        log::info!("{} ping-resp, sn: {}/{}, seq: {}.", self, resp.sn_peer_id.to_string(), from.to_string(), resp.seq.value());

        let client = self.clients.read().unwrap().get(&resp.sn_peer_id).map(|c| c.clone());
        let (new_endpoint, is_resend_immediate) = match client.as_ref() {
            None => {
                log::warn!("{} ping-resp, sn: {}/{} not found, maybe is stopped.", self, resp.sn_peer_id.to_string(), from.to_string());
                return Err(BuckyError::new(BuckyErrorCode::ErrorState, "the sn maybe is removed"));
            },
            Some(client) => {
                client.on_ping_resp(resp, &from, from_interface)
            }
        };

        if new_endpoint > UpdateOuterResult::None {
            log::info!("{} ping-resp, sn: {}/{} get new endpoint will update the desc and resend ping.", self, resp.sn_peer_id.to_string(), from.to_string());

            let stack = Stack::from(&self.env.stack);
            let clients: Vec<Arc<Client>> = self.clients.read().unwrap().iter().map(|c| c.1.clone()).collect();
            async_std::task::spawn(async move {
                if new_endpoint == UpdateOuterResult::Update {
                    stack.update_local().await;
                } else if new_endpoint == UpdateOuterResult::Reset {
                    stack.reset_local().await;
                }
                for client in clients {
                    client.local_updated();
                    client.send_ping();
                }
            });
        } else if is_resend_immediate {
            client.unwrap().send_ping();
        }

        // <TODO>持久化

        Ok(())
    }

    pub fn resend_ping(&self) {
        let clients: Vec<Arc<Client>> = self.clients.read().unwrap().iter().map(|c| c.1.clone()).collect();
        for client in clients {
            client.local_updated();
            client.send_ping();
        }
    }

    pub fn on_called(&self, called: &SnCalled, in_box: &PackageBox, from: &Endpoint, from_interface: Interface) -> Result<(), BuckyError> {
        if &called.to_peer_id != Stack::from(&self.env.stack).local_device_id() {
            log::warn!("{} called, recv called to other: {}.", self, called.to_peer_id.to_string());
            return Err(BuckyError::new(BuckyErrorCode::AddrNotAvailable, "called to other"));
        }

        log::info!("{} called, sn: {}, from: {}, seq: {}, from-eps: {}.",
            self, 
            called.sn_peer_id.to_string(),
            called.peer_info.desc().device_id().to_string(),
            called.seq.value(),
            called.peer_info.connect_info().endpoints().iter().map(|ep| ep.to_string()).collect::<Vec<String>>().concat());

        let client = self.clients.read().unwrap().get(&called.sn_peer_id).map(|c| c.clone());

        let stack = Stack::from(&self.env.stack);
        let called = called.clone();
        let key = in_box.key().clone();
        let from = from.clone();
        task::spawn(async move {
            let peer_info = &called.peer_info;
            if let Some(sigs) = peer_info.signs().body_signs() {
                if let Some(sig) = sigs.get(0) {
                    match verify_object_body_sign(&RsaCPUObjectVerifier::new(peer_info.desc().public_key().clone()), peer_info, sig).await {
                        Ok(is_ok) if is_ok => {},
                        _ => {
                            log::warn!("{} sn-called verify failed, from {:?}", stack.local_device_id(), from);
                            return;
                        }
                    }
                }
            }

            if let Ok(_) = PingClientCalledEvent::on_called(&stack, &called, ()) {
                match client {
                    None => log::warn!("{} the sn maybe is removed when recv called-req.", stack.local_device_id()),
                    Some(client) => {
                        client.on_called(&called, &key, called.call_seq, bucky_time_to_system_time(called.call_send_time), &from, from_interface);
                    }
                };
            }
        });

        // <TODO>持久化

        Ok(())
    }

    pub fn is_cached(&self, sn_device_id: &DeviceId) -> bool {
        let clients = self.clients.read().unwrap();
        match clients.get(sn_device_id) {
            Some(c) => {
                c.is_cached()
            }
            None => false
        }
    }

    pub fn reset(&self) {
        let clients: Vec<Arc<Client>> = self.clients.read().unwrap().iter().map(|kv| kv.1.clone()).collect();
        for c in clients {
            c.reset();
        }
    }

    pub async fn wait_online(&self, sn: &DeviceId) -> bool {
        let client = {
            self.clients.read().unwrap()
                .get(sn)
                .cloned()
        };

        if let Some(client) = client {
            client.wait_online().await
        } else {
            false
        }
    }
}

const PING_CLIENT_STATUS_INIT: u8 = 0;
const PING_CLIENT_STATUS_RUNNING: u8 = 1;
const PING_CLIENT_STATUS_BUSY: u8 = 2;
const PING_CLIENT_STATUS_STOPPING: u8 = 3;
const PING_CLIENT_STATUS_STOPPED: u8 = 4;

const SN_STATUS_INIT: u8 = 0;
const SN_STATUS_CONNECTING: u8 = 1;
const SN_STATUS_ONLINE: u8 = 2;
const SN_STATUS_OFFLINE: u8 = 3;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SnStatus {
    Init = 0, 
    Connecting, 
    Online, 
    Offline
}

impl TryFrom<u8> for SnStatus {
    type Error = BuckyError;
    fn try_from(code: u8) -> BuckyResult<Self> {
        match code {
            SN_STATUS_INIT => Ok(Self::Init), 
            SN_STATUS_CONNECTING => Ok(Self::Connecting), 
            SN_STATUS_ONLINE => Ok(Self::Online), 
            SN_STATUS_OFFLINE => Ok(Self::Offline), 
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidParam, "error sn status code"))
        }
    }
}

impl std::fmt::Display for SnStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let v = match self {
            Self::Init => "init",
            Self::Connecting => "connecting",
            Self::Online => "online",
            Self::Offline => "offline",
        };

        write!(f, "{}", v)
    }
}

impl std::str::FromStr for SnStatus {
    type Err = BuckyError;

    fn from_str(s: &str) -> BuckyResult<Self> {
        match s {
            "init" => Ok(Self::Init),
            "connecting" => Ok(Self::Connecting),
            "online" => Ok(Self::Online),
            "offline" => Ok(Self::Offline),
            _ => {
                let msg = format!("unknown SnStatus value: {}", s);
                log::error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidData, msg))
            }
        }
    }
}


#[derive(Clone)]
struct Client {
    inner: Arc<ClientInner>,
    ping_trigger: (async_std::channel::Sender<()>, async_std::channel::Receiver<()>),
}

impl Client {
    fn new(mgr: &PingManager, sn: &Device, _is_encrypto: bool, appraiser: Box<dyn ServiceAppraiser>) -> Client {
        let mut last_receipt = SnServiceReceipt::default();
        last_receipt.version = SnServiceReceiptVersion::Invalid;
        last_receipt.start_time = UNIX_EPOCH;
        let mut receipt = SnServiceReceipt::default();
        receipt.version = SnServiceReceiptVersion::Current;
        receipt.start_time = SystemTime::now();

        let sn_peerid = sn.desc().device_id();
        let mut sessions = Vec::default();
        let net_listener = Stack::from(&mgr.env.stack).net_manager().listener().clone();
        for udp in net_listener.udp() {
            let session = Session {
                net_listener: net_listener.clone(), 
                interface: udp.clone(),
                sn: sn.clone(),
                remote_endpoint: RwLock::new(None),
                last_ping_time: AtomicU64::new(0),
                last_resp_time: AtomicU64::new(0)
            };
            sessions.push(session);
        }

        let inner = ClientInner {
            env: mgr.env.clone(),
            create_time: Instant::now(),
            sn_peerid: sn_peerid.clone(),
            sn: sn.clone(),
            sessions: RwLock::new(sessions),
            active_session_index: AtomicU32::new(std::u32::MAX),
            client_status: AtomicU8::new(PING_CLIENT_STATUS_INIT),
            ping_status: RwLock::new(PingState::Connecting(StateWaiter::new())),
            sn_status: AtomicU8::new(SN_STATUS_INIT),
            last_ping_time: AtomicU64::new(0),
            last_resp_time: AtomicU64::new(0),
            last_update_seq: AtomicU32::new(1),
            seq_genarator: TempSeqGenerator::new(),

            contract: Contract {
                sn_peerid: sn_peerid,
                sn: sn.clone(),
                stat: Mutex::new(ContractStat {
                    commit_receipt_start_time: UNIX_EPOCH,
                    last_receipt: last_receipt,
                    receipt: receipt,
                    last_call_peers: Default::default(),
                    call_peers: Default::default(),
                }),
                wait_seq: AtomicU32::new(0),
                appraiser: Arc::new(appraiser)
            }
        };


        Client {
            inner: Arc::new(inner),
            ping_trigger: async_std::channel::bounded(8),
        }
    }

    pub fn status(&self) -> SnStatus {
        SnStatus::try_from(self.inner.sn_status.load(atomic::Ordering::SeqCst)).unwrap()
    }

    fn reset(&self) {
        let mut sessions = self.inner.sessions.write().unwrap();
        *sessions = Default::default();
        self.inner.active_session_index.store(std::u32::MAX, atomic::Ordering::Release);
        // self.inner.client_status.store(PING_CLIENT_STATUS_INIT, atomic::Ordering::Release);
        self.inner.last_ping_time.store(0, atomic::Ordering::Release);
        self.inner.last_resp_time.store(0, atomic::Ordering::Release);
        self.inner.last_update_seq.store(1, atomic::Ordering::Release);

        {
            let state = &mut *self.inner.ping_status.write().unwrap();
            match state {
                PingState::Connecting(_) => {},
                PingState::Online => {
                    *state = PingState::Connecting(StateWaiter::new());
                }
            }
        }

        let net_listener = Stack::from(&self.inner.env.stack).net_manager().listener().clone();
        for udp in net_listener.udp() {
            let session = Session {
                net_listener: net_listener.clone(), 
                interface: udp.clone(),
                sn: self.inner.sn.clone(),
                remote_endpoint: RwLock::new(None),
                last_ping_time: AtomicU64::new(0),
                last_resp_time: AtomicU64::new(0)
            };
            sessions.push(session);
        }

        if self.inner.client_status.load(atomic::Ordering::Acquire) != PING_CLIENT_STATUS_INIT {
            self.inner.sn_status.store(SN_STATUS_CONNECTING, atomic::Ordering::Release);
            let ping_trigger = self.ping_trigger.0.clone();
            task::spawn(async move {
                let _ = ping_trigger.send(()).await.map_err(|e|{
                    log::error!("Client reset, ping_trigger.send err: {}", e);
                });
            });
        }
    }

    fn start(&self) {
        let inner = self.inner.clone();

        let old_status: u8 = inner.client_status.compare_exchange(PING_CLIENT_STATUS_INIT, PING_CLIENT_STATUS_RUNNING, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst).unwrap();
        assert_eq!(old_status, PING_CLIENT_STATUS_INIT, "ping client start in invalid status({})", old_status);

        let old_status: u8 = inner.sn_status.compare_exchange(SN_STATUS_INIT, SN_STATUS_CONNECTING, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst).unwrap();
        assert_eq!(old_status, SN_STATUS_INIT, "ping client start in invalid status({})", old_status);

        let ping_trigger = self.ping_trigger.1.clone();

        task::spawn(async move {
            loop {
                match inner.client_status.compare_exchange(
                    PING_CLIENT_STATUS_RUNNING, 
                    PING_CLIENT_STATUS_BUSY, 
                    atomic::Ordering::SeqCst, 
                    atomic::Ordering::SeqCst) {
                    Ok(_) => {

                    }, 
                    Err(old_status) => {
                        if old_status == PING_CLIENT_STATUS_STOPPED || old_status == PING_CLIENT_STATUS_STOPPING {
                            break;
                        }
                    }
                };
                
                inner.update_status();
                let ping_interval_ms = match inner.sn_status.load(atomic::Ordering::SeqCst) {
                    SN_STATUS_CONNECTING => inner.env.ping_interval_init_ms,
                    SN_STATUS_ONLINE | SN_STATUS_OFFLINE => inner.env.ping_interval_ms,
                    _ => {
                        assert!(false, "won't reach here.");
                        inner.env.ping_interval_ms
                    }
                };

                let now = inner.create_time.elapsed().as_millis() as u64;
                let last_ping_time = inner.last_ping_time.load(atomic::Ordering::Acquire);
                if last_ping_time == 0 || now < last_ping_time || now - last_ping_time >= ping_interval_ms as u64 {
                    let _ = inner.send_ping().await;
                }

                // <TODO>持久化服务证明

                let _ = inner.client_status.compare_exchange(PING_CLIENT_STATUS_BUSY, PING_CLIENT_STATUS_RUNNING, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst).unwrap();

                // wait
                let waiter = ping_trigger.clone();
                let _ = async_std::io::timeout(Duration::from_millis((ping_interval_ms >> 1u32) as u64), async move {
                    waiter.recv().await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
                }).await;
            }
        });
    }

    fn stop(&self) {
        // 等正在执行的任务结束
        loop {
            // FIXME: 这里不会死循环？
            if let Err(state) = self.inner.client_status.compare_exchange(PING_CLIENT_STATUS_RUNNING, PING_CLIENT_STATUS_STOPPING, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst) {
                if state == PING_CLIENT_STATUS_BUSY {
                    continue;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn local_updated(&self) {
        self.inner.last_update_seq.store(1, atomic::Ordering::Release);
    }

    fn on_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface) -> (UpdateOuterResult, bool) {
        self.inner.on_ping_resp(resp, from, from_interface)
    }

    fn send_ping(&self) {
        let inner = self.inner.clone();
        task::spawn(async move {
            let _ = inner.send_ping().await;
        });
    }

    pub fn on_called(&self, called: &SnCalled, key: &MixAesKey, call_seq: TempSeq, call_time: SystemTime, from: &Endpoint, from_interface: Interface) {
        let _ = self.inner.on_called(called, key, call_seq, call_time, from, from_interface);
    }

    fn is_cached(&self) -> bool {
        self.inner.last_update_seq.load(atomic::Ordering::Acquire) == 0
    }

    pub async fn wait_online(&self) -> bool {
        let waiter = {
            let state = &mut *self.inner.ping_status.write().unwrap();
            match state {
                PingState::Connecting(waiter) => Some(waiter.new_waiter()),
                PingState::Online => None,
            }
        };

        if let Some(waiter) = waiter {
            StateWaiter::wait(waiter, || true).await
        } else {
            true
        }
    }
}

enum PingState {
    Connecting(StateWaiter),
    Online,
}

struct ClientInner {
    env: Arc<PingEnv>,
    create_time: Instant, // Client对象构造时间，所有计时器以此为偏移计数

    sn_peerid: DeviceId,
    sn: Device,

    sessions: RwLock<Vec<Session>>,
    active_session_index: AtomicU32,

    ping_status: RwLock<PingState>,
    client_status: AtomicU8,
    sn_status: AtomicU8,

    last_ping_time: AtomicU64,
    last_resp_time: AtomicU64,
    last_update_seq: AtomicU32,

    seq_genarator: TempSeqGenerator,

    contract: Contract,
}

impl std::fmt::Display for ClientInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stack = Stack::from(&self.env.stack);
        write!(f, "PingManager{{local:{}}}", stack.local_device_id())
    }
}

impl ClientInner {
    fn update_status(&self) {
        let now = self.create_time.elapsed().as_millis() as u64;
        let env = self.env.clone();
        let last_resp_time = self.last_resp_time.load(atomic::Ordering::Acquire);
        let last_ping_time = self.last_ping_time.load(atomic::Ordering::Acquire);

        let sn_status = self.sn_status.load(atomic::Ordering::SeqCst);
        match sn_status {
            SN_STATUS_INIT => assert!(false, "won't reach here."),
            SN_STATUS_CONNECTING => {
                if last_ping_time != 0 {
                    let cur_status = if last_resp_time != 0 {
                        if now < last_resp_time || now - last_resp_time < env.offline_ms as u64 {
                            // online
                            if self.sn_status.compare_exchange(SN_STATUS_CONNECTING, SN_STATUS_ONLINE, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst).is_ok() {
                                self.online(&self.sn);
                            }
                            SN_STATUS_ONLINE
                        } else {
                            // offline
                            SN_STATUS_OFFLINE
                        }
                    } else {
                        SN_STATUS_CONNECTING
                    };

                    if cur_status != SN_STATUS_ONLINE && now > last_ping_time && now - last_ping_time > env.offline_ms as u64 {
                        if self.sn_status.compare_exchange(SN_STATUS_CONNECTING, SN_STATUS_OFFLINE, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst).is_ok() {
                            self.offline(&self.sn);
                        }
                    }
                }
            }
            SN_STATUS_OFFLINE => {
                if last_resp_time != 0 {
                    if now < last_resp_time || now - last_resp_time < env.offline_ms as u64 {
                        if self.sn_status.compare_exchange(SN_STATUS_OFFLINE, SN_STATUS_ONLINE, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst).is_ok() {
                            self.online(&self.sn);
                        }
                    }
                }
            }
            SN_STATUS_ONLINE => {
                assert!(last_resp_time > 0);
                if now > last_resp_time && now - last_resp_time > env.offline_ms as u64 {
                    if self.sn_status.compare_exchange(SN_STATUS_ONLINE, SN_STATUS_OFFLINE, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst).is_ok() {
                        self.offline(&self.sn);
                    }
                }
            }
            _ => {
                assert!(false, "unknown sn-status({})", sn_status);
            }
        }
    }

    fn on_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface) -> (UpdateOuterResult/*has new endpoint*/, bool/*is_resend_immdiate*/) {
        let now = self.create_time.elapsed().as_millis() as u64;
        self.last_resp_time.store(now, atomic::Ordering::Release);

        let mut rto = 0;
        let mut is_handled = false;

        let active_session_index = self.active_session_index.load(atomic::Ordering::Acquire) as usize;
        let sessions = self.sessions.read().unwrap();
        let try_session = sessions.get(active_session_index);
        let mut new_endpoint = UpdateOuterResult::None;
        if let Some(s) = try_session {
            let r = s.on_ping_resp(resp, from, from_interface.clone(), now, &mut rto, &mut is_handled);
            new_endpoint = std::cmp::max(r, new_endpoint);
        }

        if !is_handled {
            let mut index = 0;
            for session in (*sessions).as_slice() {
                let r = session.on_ping_resp(resp, from, from_interface.clone(), now, &mut rto, &mut is_handled);
                new_endpoint = std::cmp::max(r, new_endpoint);
                if is_handled {
                    let _ = self.active_session_index.compare_exchange(std::u32::MAX, index, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst);
                    break;
                }
                index += 1;
            }
        }

        self.update_status();

        self.contract.on_ping_resp(resp, rto);

        let is_resend_immdiate = if resp.result == BuckyErrorCode::NotFound.as_u8() {
            // 要更新desc
            let _ = self.last_update_seq.compare_exchange(0, 1, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst);
            true
        } else {
            let _ = self.last_update_seq.compare_exchange(resp.seq.value(), 0, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst);
            false
        };

        (new_endpoint, is_resend_immdiate)
    }

    async fn send_ping(&self) -> BuckyResult<()> {
        let now = self.create_time.elapsed().as_millis() as u64;
        let now_abs = SystemTime::now();
        let now_abs_u64 = bucky_time_now();
        let seq = self.seq_genarator.generate();

        let stack = Stack::from(&self.env.stack);
        let local_peer = stack.device_cache().local();
        
        let last_resp_time = self.last_resp_time.load(atomic::Ordering::Acquire);
        let to_session_index = if last_resp_time == 0 || (now < last_resp_time || now - last_resp_time > 1000) {
            self.active_session_index.store(std::u32::MAX, atomic::Ordering::Release);
            std::u32::MAX
        } else {
            self.active_session_index.load(atomic::Ordering::Acquire)
        };

        let to_sessions = {
            let sessions = self.sessions.read().unwrap();
            let to_session = if to_session_index != std::u32::MAX {
                sessions.get(to_session_index as usize)
            } else {
                None
            };

            match to_session {
                Some(s) => vec![s.prepare_send_endpoints(now_abs_u64)],
                None => (*sessions).iter().map(|s| s.prepare_send_endpoints(now_abs_u64)).collect()
            }
        };

        if to_sessions.len() == 0 {
            return Err(BuckyError::new(BuckyErrorCode::NotFound, "no ping target"));
        }

        let ping_pkg = {
            let stack = Stack::from(&self.env.stack);
            let last_update_seq = self.last_update_seq.swap(seq.value(), atomic::Ordering::AcqRel);
            let mut ping_pkg = SnPing {
                protocol_version: 0, 
                stack_version: 0, 
                seq,
                from_peer_id: Some(stack.local_device_id().clone()),
                sn_peer_id: self.sn_peerid.clone(),
                peer_info: if last_update_seq != 0 { Some(local_peer.clone()) } else { None }, // 本地信息更新了信息，需要同步，或者服务器要求更新
                send_time: now_abs_u64,
                contract_id: None, // <TODO>
                receipt: None
            };

            match &ping_pkg.peer_info {
                Some(dev) => log::info!("{} ping-req seq: {:?}, endpoints: {:?}", self,  ping_pkg.seq, dev.connect_info().endpoints()),
                None => log::debug!("{} ping-req seq: {:?}, endpoints: none", self, ping_pkg.seq),
            }
            // 填充receipt
            self.contract.prepare_receipt(&mut ping_pkg, now_abs, stack.keystore().private_key());
            ping_pkg
        };

        let key_stub = stack.keystore().create_key(self.sn.desc(), true);

        let mut pkg_box = PackageBox::encrypt_box(
            self.sn_peerid.clone(), 
            key_stub.key.clone());

        if let keystore::EncryptedKey::Unconfirmed(key_encrypted) = key_stub.encrypted {
            let stack = Stack::from(&self.env.stack);
            let mut exchg = Exchange::from((&ping_pkg, local_peer.clone(), key_encrypted, key_stub.key.mix_key));
            let _ = exchg.sign(stack.keystore().signer()).await;
            pkg_box.push(exchg);
        }

        let ping_seq = ping_pkg.seq.clone();
        pkg_box.push(ping_pkg);

        let mut context = PackageBoxEncodeContext::default();

        self.last_ping_time.store(now, atomic::Ordering::Release);

        self.contract.will_ping(seq.value());

        struct SendIter {
            sessions: Vec<(Interface, Vec<Endpoint>)>,
            sub_pos: usize,
            pos: usize,
        }

        impl Iterator for SendIter {
            type Item = (Interface, Endpoint);

            fn next(&mut self) -> Option<Self::Item> {
                let sessions = self.sessions.get(self.pos);
                if let Some((from, to_endpoints)) = sessions {
                    let ep = to_endpoints.get(self.sub_pos);
                    if let Some(ep) = ep {
                        self.sub_pos += 1;
                        Some(((*from).clone(), ep.clone()))
                    } else {
                        self.pos += 1;
                        self.sub_pos = 0;
                        self.next()
                    }
                } else {
                    None
                }
            }
        }

        let send_iter = SendIter {
            sessions: to_sessions,
            sub_pos: 0,
            pos: 0
        };
        Interface::send_box_mult(&mut context,
                                         &pkg_box,
                                         send_iter,
                                         |from, to, result| {
                                             log::debug!("{} ping seq:{:?} from {} to {}/{}, result: {:?}", self, ping_seq, from.local(), self.sn_peerid.to_string(), to, result);
                                             true
                                         })?;
        Ok(())
    }

    fn on_called(&self, called: &SnCalled, key: &MixAesKey, call_seq: TempSeq, call_time: std::time::SystemTime, from: &Endpoint, from_interface: Interface) -> Result<(), BuckyError> {
        let resp = SnCalledResp {
            seq: called.seq,
            result: 0,
            sn_peer_id: self.sn_peerid.clone(),
        };

        

        let mut pkg_box = PackageBox::encrypt_box(
            self.sn_peerid.clone(), 
            key.clone());
        pkg_box.push(resp);

        let mut context = PackageBoxEncodeContext::default();
        let _ = from_interface.send_box_to(&mut context, &pkg_box, from)?;

        self.contract.on_called(called, call_seq, call_time);

        Ok(())
    }
}

impl PingClientStateEvent for ClientInner{
    fn online(&self, sn: &Device) {
        PingClientStateEvent::online(&Stack::from(&self.env.stack), sn);

        let waker = {
            let state = &mut *self.ping_status.write().unwrap();
            match state {
                PingState::Connecting(waker) => {
                    let waker = waker.transfer();
                    *state = PingState::Online;
                    Some(waker)
                },
                PingState::Online => None,
            }
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }

    fn offline(&self, sn: &Device) {
        PingClientStateEvent::offline(&Stack::from(&self.env.stack), sn);

        {
            let state = &mut *self.ping_status.write().unwrap();
            match state {
                PingState::Connecting(_) => {},
                PingState::Online => {
                    *state = PingState::Connecting(StateWaiter::new());
                }
            }
        }
    }
}

struct Session {
    net_listener: NetListener, 
    interface: Interface,
    sn: Device,
    remote_endpoint: RwLock<Option<Endpoint>>, // 对端(SN)地址
    last_ping_time: AtomicU64,
    last_resp_time: AtomicU64,
}

impl Session {
    fn prepare_send_endpoints(&self, now: u64) -> (Interface, Vec<Endpoint>) {
        let local_endpoint = self.interface.local();
        let to_endpoints = {
            let (eps, is_reset) = {
                let remote_endpoint = self.remote_endpoint.read().unwrap();
                let last_resp_time = self.last_resp_time.load(atomic::Ordering::Acquire);
                let last_ping_time = self.last_ping_time.load(atomic::Ordering::Acquire);
                if last_resp_time == 0 || remote_endpoint.is_none() || (now > last_ping_time && now - last_ping_time > 1000 && last_ping_time > last_resp_time) {
                    (self.sn.connect_info().endpoints().iter().filter(|ep| ep.is_same_ip_version(&local_endpoint) && ep.is_udp()).map(|ep| ep.clone()).collect(),
                    true)
                } else {
                    (vec![remote_endpoint.unwrap().clone()],
                    false)
                }
            };

            if is_reset {
                *self.remote_endpoint.write().unwrap() = None;
            }
            eps
        };

        self.last_ping_time.store(now, atomic::Ordering::Release);

        (self.interface.clone(), to_endpoints)
    }

    fn on_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface, now: u64, rto: &mut u16, is_handled: &mut bool) -> UpdateOuterResult {
        if !self.interface.is_same(&from_interface) {
            *is_handled = false;
            return UpdateOuterResult::None;
        }
        *is_handled = true;

        if resp.end_point_array.len() == 0 {
            return UpdateOuterResult::None;
        }

        let is_remote_none = self.remote_endpoint.read().unwrap().is_none();
        if is_remote_none {
            *self.remote_endpoint.write().unwrap() = Some(from.clone());
        }
        self.last_resp_time.store(now, atomic::Ordering::Release);
        let last_ping_time = self.last_ping_time.load(atomic::Ordering::Acquire);
        if now > last_ping_time {
            *rto = (now - last_ping_time) as u16;
        } else {
            *rto = 0;
        }

        let out_endpoint = resp.end_point_array.get(0).unwrap();
        self.net_listener.update_outer(&self.interface.local(), &out_endpoint)
    }
}

struct Contract {
    sn_peerid: DeviceId,
    sn: Device,
    stat: Mutex<ContractStat>,
    wait_seq: AtomicU32,
    appraiser: Arc<Box<dyn ServiceAppraiser>>,
}

#[derive(Clone)]
struct CallPeerStat {
    peerid: DeviceId,
    last_seq: TempSeq,
    is_connect_success: bool,
}

struct ContractStat {
    commit_receipt_start_time: SystemTime,
    last_receipt: SnServiceReceipt,
    receipt: SnServiceReceipt,
    last_call_peers: HashMap<DeviceId, CallPeerStat>,
    call_peers: HashMap<DeviceId, CallPeerStat>,
}

impl Contract {
    fn on_ping_resp(&self, resp: &SnPingResp, rto: u16) {
        if let Ok(wait_seq) = self.wait_seq.compare_exchange(resp.seq.value(), 0, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst) {
            if wait_seq != 0 {
                // 统计并获取当前服务清单
                let (receipt, last_receipt) = {
                    let mut stat = self.stat.lock().unwrap();
                    let receipt = &mut stat.receipt;
                    receipt.ping_resp_count += 1;
                    if rto > 0 {
                        receipt.rto = ((receipt.rto as u32 * 7 + rto as u32) / 8) as u16;
                    }

                    match resp.receipt.as_ref() {
                        Some(_) => {
                            let last_receipt = &mut stat.last_receipt;
                            let cloned_last_receipt = match last_receipt.version {
                            SnServiceReceiptVersion::Invalid => None,
                            SnServiceReceiptVersion::Current => Some((*last_receipt).clone())
                            };
                            (Some(stat.receipt.clone()), cloned_last_receipt)
                        }
                        None => (None, None)
                    }
                };

                if let Some(sn_receipt) = resp.receipt.as_ref() {
                    let grade = self.appraiser.appraise(&self.sn, &receipt, &last_receipt, &Some(sn_receipt.clone()));
                    let mut stat = self.stat.lock().unwrap();
                    stat.receipt.grade = grade;
                    stat.commit_receipt_start_time = sn_receipt.start_time;
                }
            }
        }
    }

    fn on_called(&self, called: &SnCalled, seq: TempSeq, call_time: SystemTime) {
        let now = SystemTime::now();
        let mut stat = self.stat.lock().unwrap();
        let receipt = &mut stat.receipt;
        if now > call_time {
            let delay = now.duration_since(call_time).unwrap().as_millis() as u16;
            receipt.call_delay = ((receipt.call_delay as u32 * 7 + delay as u32) / 8) as u16
        }

        let (called_inc, call_peer_inc) = match stat.call_peers.entry(called.peer_info.desc().device_id()) {
            Entry::Occupied(exist) => {
                let exist = exist.into_mut();
                if exist.last_seq != seq {
                    exist.last_seq = seq;
                    (1, 0)
                } else {
                    (0, 0)
                }
            }
            Entry::Vacant(entry) => {
                let init_stat = CallPeerStat {
                    peerid: called.peer_info.desc().device_id(),
                    last_seq: seq,
                    is_connect_success: false
                };
                entry.insert(init_stat);
                (1, 1)
            }
        };

        stat.receipt.called_count += called_inc;
        stat.receipt.call_peer_count += call_peer_inc;
    }

    fn prepare_receipt(&self, ping_pkg: &mut SnPing, now_abs: SystemTime, secret: &PrivateKey) {
        let mut stat = self.stat.lock().unwrap();
        if stat.commit_receipt_start_time > UNIX_EPOCH && stat.receipt.grade.is_accept() {
            let commit_receipt_start_time = stat.commit_receipt_start_time;
            if stat.last_receipt.version != SnServiceReceiptVersion::Invalid &&
                commit_receipt_start_time <= stat.last_receipt.start_time {
                stat.last_receipt.grade = stat.receipt.grade;
                stat.last_receipt.rto = stat.receipt.rto;
                stat.last_receipt.ping_count += stat.receipt.ping_count;
                stat.last_receipt.ping_resp_count += stat.receipt.ping_resp_count;
                stat.last_receipt.called_count += stat.receipt.called_count;
                stat.last_receipt.call_delay = stat.receipt.call_delay;

                // 合并call peer
                let mut add_succ_peer_count = 0u32;
                let mut add_peer_ount = 0u32;
                let mut cur_call_peers = Default::default();
                std::mem::swap(&mut stat.call_peers, &mut cur_call_peers);
                for cur in cur_call_peers.values() {
                    match stat.last_call_peers.entry(cur.peerid.clone()) {
                        Entry::Occupied(entry) => {
                            let mut last = entry.into_mut();
                            last.last_seq = cur.last_seq;
                            if cur.is_connect_success && !last.is_connect_success {
                                last.is_connect_success = true;
                                add_succ_peer_count += 1;
                            }
                        }
                        Entry::Vacant(entry) => {
                            entry.insert((*cur).clone());
                            add_peer_ount += 1;
                            if cur.is_connect_success {
                                add_succ_peer_count += 1;
                            }
                        }
                    }
                }

                let last_receipt = &mut stat.last_receipt;
                last_receipt.call_peer_count += add_peer_ount;
                last_receipt.connect_peer_count += add_succ_peer_count;
            } else {
                let mut cur_call_peers = Default::default();
                std::mem::swap(&mut stat.call_peers, &mut cur_call_peers);
                std::mem::swap(&mut stat.last_call_peers, &mut cur_call_peers);
                stat.last_receipt = stat.receipt.clone();
            }

            if let Ok(d) = now_abs.duration_since(stat.last_receipt.start_time) {
                stat.last_receipt.duration = d;
            }

            // 重置正在进行的统计
            stat.receipt.ping_count = 0;
            stat.receipt.ping_resp_count = 0;
            stat.receipt.called_count = 0;
            stat.receipt.call_peer_count = 0;
            stat.receipt.connect_peer_count = 0;
            stat.receipt.start_time = now_abs;

            // 签名
            let sign = match stat.last_receipt.sign(&ping_pkg.sn_peer_id, secret) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!("sign for receipt failed, err: {:?}", e);
                    return;
                }
            };
            ping_pkg.receipt = Some(ReceiptWithSignature::from((stat.last_receipt.clone(), sign)));

            stat.commit_receipt_start_time = UNIX_EPOCH;
        }
    }

    fn will_ping(&self, seq: u32) {
        self.wait_seq.store(seq, atomic::Ordering::SeqCst);
        self.stat.lock().unwrap().receipt.ping_count += 1;
    }
}

fn is_new_endpoint(desc: &Device, ep: &Endpoint) -> bool {
    for cur in desc.connect_info().endpoints() {
        if cur.is_udp() && cur.addr() == ep.addr() {
            return false;
        }
    }
    true
}
