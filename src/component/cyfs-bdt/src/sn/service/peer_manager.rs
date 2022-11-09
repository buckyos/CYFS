use std::{
    collections::{HashMap, hash_map}, 
    time::Duration, 
    sync::{Arc, atomic::{AtomicU64, Ordering}}
};
use cyfs_debug::Mutex;
use cyfs_base::*;
use crate::{
    types::*, 
    sn::Config,
};
use super::{
    net_listener::UdpSender, statistic::{PeerStatus, StatisticManager, }
};

pub struct FoundPeer {
    pub desc: Device,
    pub sender: Arc<UdpSender>,
    pub is_wan: bool,
    pub peer_status: PeerStatus,
}


struct CachedPeerInfo {
    pub desc: Device,
    pub sender: Arc<UdpSender>,
    pub aes_key: Option<MixAesKey>,
    pub last_send_time: Timestamp,
    pub last_call_time: Timestamp,
    pub is_wan: bool,

    pub last_ping_seq: TempSeq,

    pub peer_status: PeerStatus,
    // pub call_peers: HashMap<DeviceId, TempSeq>, // <peerid, last_call_seq>
    // pub receipt: SnServiceReceipt,
    // pub last_receipt_request_time: ReceiptRequestTime,
}

fn has_wan_endpoint(desc: &Device) -> bool {
    for ep in desc.connect_info().endpoints() {
        if ep.is_static_wan() {
            return true;
        }
    }
    false
}

fn contain_addr(dev: &Device, addr: &SocketAddr) -> bool {
    let endpoints: &Vec<Endpoint> = dev.connect_info().endpoints();
    for endpoint in endpoints {
        if endpoint.addr() == addr {
            return true;
        }
    }
    false
}

impl CachedPeerInfo {
    fn new(
        desc: Device, 
        sender: Arc<UdpSender>, 
        aes_key: Option<&MixAesKey>, 
        send_time: Timestamp, 
        seq: TempSeq, 
        peer_status: PeerStatus) -> CachedPeerInfo {
        CachedPeerInfo {
            is_wan: has_wan_endpoint(&desc),
            last_ping_seq: seq,
            desc,
            sender,
            aes_key: aes_key.map(|k| k.clone()),
            last_send_time: send_time,
            last_call_time: 0,
            peer_status,
            // call_peers: Default::default(),
            // receipt: Default::default(),
            // last_receipt_request_time: ReceiptRequestTime::None,
        }
    }

    fn to_found_peer(&self) -> FoundPeer {
        FoundPeer {
            desc: self.desc.clone(), 
            sender: self.sender.clone(), 
            is_wan: self.is_wan,
            peer_status: self.peer_status.clone(),
        }
    }

    fn update_key(&mut self, aes_key: &MixAesKey) {
        if let Some(k) = self.aes_key.as_mut() {
            *k = aes_key.clone();
        } else {
            self.aes_key = Some(aes_key.clone());
        }
    }

    fn update_desc(&mut self, desc: &Device) -> BuckyResult<bool> {
        match desc.signs().body_signs() {
            Some(sigs) if !sigs.is_empty() => {
                if let Some(old_sigs) = self.desc.signs().body_signs() {
                    if let Some(old_sig) = old_sigs.get(0) {
                        let new_sig = sigs.get(0).unwrap();
                        match new_sig.sign_time().cmp(&old_sig.sign_time()) {
                            std::cmp::Ordering::Equal => return Ok(false), // 签名时间不变，不更新
                            std::cmp::Ordering::Less => return Err(BuckyError::new(BuckyErrorCode::Expired, "sign expired")), // 签名时间更早，忽略
                            std::cmp::Ordering::Greater => {}
                        }
                    }
                }
            },
            _ => match self.desc.signs().body_signs() {
                Some(sigs) if !sigs.is_empty() => return Err(BuckyError::new(BuckyErrorCode::NotMatch, "attempt update signed-object with no-signed")), // 未签名不能取代签名device信息，要等被淘汰后才生效
                _ => {},
            }
        };

        self.desc = desc.clone();
        self.is_wan = has_wan_endpoint(desc);
        Ok(true)
    }
}

struct Peers {
    active_peers: HashMap<DeviceId, CachedPeerInfo>,
    knock_peers: HashMap<DeviceId, CachedPeerInfo>,
}

impl Peers {
    fn find_peer(&mut self, peerid: &DeviceId, reason: FindPeerReason) -> Option<&mut CachedPeerInfo> {
        let found_cache = match self.active_peers.get_mut(peerid) {
            Some(p) => {
                Some(p)
            },
            None => match self.knock_peers.get_mut(peerid) {
                Some(p) => Some(p),
                None => None
            }
        };
    
        if let Some(p) = found_cache {
            match reason {
                FindPeerReason::CallFrom(t) => {
                    if t > p.last_call_time {
                        p.last_call_time = t;
                    }
                    Some(p)
                },
                FindPeerReason::Other => {
                    Some(p)
                }
            }
        } else {
            None
        }
    }
}


pub struct PeerManager {
    peers: Mutex<Peers>, 
    last_knock_time: AtomicU64,
    timeout: Duration,
    config: Config,
    statistic_manager: StatisticManager,
}

enum FindPeerReason {
    CallFrom(Timestamp),
    Other,
}


impl PeerManager {
    pub fn new(timeout: Duration, config: Config) -> PeerManager {
        PeerManager {
            peers: Mutex::new(Peers {
                active_peers: Default::default(),
                knock_peers: Default::default(),
            }),
            last_knock_time: AtomicU64::new(bucky_time_now()),
            timeout,
            config,
            statistic_manager: StatisticManager::default(),
        }
    }

    pub fn peer_heartbeat(
        &self, 
        peerid: DeviceId, 
        peer_desc: &Option<Device>, 
        sender: Arc<UdpSender>, 
        aes_key: Option<&MixAesKey>, 
        send_time: Timestamp, 
        seq: TempSeq) -> bool {

        let exist_cache_found = |cached_peer: &mut CachedPeerInfo| -> bool {
            if cached_peer.last_send_time > send_time {
                log::warn!("ping send-time little.");
                return false;
            }
            if let Some(desc) = peer_desc {
                if let Err(e) = cached_peer.update_desc(desc) {
                    log::warn!("ping update device-info failed, err: {:?}", e);
                    return false;
                }
                log::debug!("ping update device-info, endpoints: {:?}", desc.connect_info().endpoints());
            } else {
                log::debug!("ping without device-info.");
            }

            match (send_time - cached_peer.last_send_time) / self.config.ping_interval.as_micros() as u64 {
                0 => {
                    cached_peer.peer_status.wait_online(send_time, 
                        if cached_peer.last_ping_seq >= seq {
                            false
                        } else {
                            (seq.value() - cached_peer.last_ping_seq.value()) > 50
                        });
                }
                _ => {
                    cached_peer.peer_status.online(seq, send_time);
                }
            }

            /*
            // statistic ping req
            match (send_time - cached_peer.last_send_time) / self.config.ping_interval_init.as_micros() as u64 {
                0 | 1 => {
                    match cached_peer.peer_status.status() {
                        PeerStatusKind::Connecting(_) => {},
                        _ => cached_peer.peer_status = self.statistic_manager.get_status(peerid.clone(), send_time),
                    }
                }
                _ => {
                    match cached_peer.peer_status.status() {
                        PeerStatusKind::Connecting(_) => {
                            cached_peer.peer_status.online(send_time);
                        }
                        _ => {},
                    }
                }
            }
            */

            cached_peer.last_send_time = send_time;
            cached_peer.last_ping_seq = seq;

            // 客户端被签名的地址才被更新，避免恶意伪装
            if contain_addr(&cached_peer.desc, sender.remote()) 
                || cached_peer.sender.key().mix_key != sender.key().mix_key {
                cached_peer.sender = sender.clone();
            }

            if let Some(k) = aes_key {
                cached_peer.update_key(k);
            }

            true
        };

        let mut peers = self.peers.lock().unwrap();
        // 1.从活跃peer中搜索已有cache
        if let Some(p) = peers.active_peers.get_mut(&peerid) {
            return exist_cache_found(p);
        }

        // 2.从待淘汰peer中搜索已有cache
        let to_active = if let hash_map::Entry::Occupied(mut entry) = peers.knock_peers.entry(peerid.clone()) {
            if !exist_cache_found(entry.get_mut()) {
                return false;
            }
            Some(entry.remove())
        } else {
            None
        };
        if let Some(to_active) = to_active {
            let old = peers.active_peers.insert(peerid.clone(), to_active);
            assert!(old.is_none());
            return true;
        }

        // 3.新建cache
        match peer_desc {
            Some(desc) => {
                let old = peers.active_peers.insert(peerid.clone(), CachedPeerInfo::new(desc.clone(), sender, aes_key, send_time, seq, self.statistic_manager.get_status(peerid.clone(), send_time)));
                assert!(old.is_none());
                true
            }
            None => false
        }
    }

    pub fn try_knock_timeout(&self, now: Timestamp) -> Option<Vec<DeviceId>> {
        let last_knock_time = self.last_knock_time.load(Ordering::SeqCst);
        let drop_maps = if now > last_knock_time && Duration::from_micros(now - last_knock_time) > self.timeout {
            let mut peers = self.peers.lock().unwrap();
            let mut knock_peers = Default::default();
            std::mem::swap(&mut knock_peers, &mut peers.active_peers);
            std::mem::swap(&mut knock_peers, &mut peers.knock_peers);
            self.last_knock_time.store(now, Ordering::SeqCst);

            Some(knock_peers.into_keys().collect())
        } else {
            None
        };

        self.statistic_manager.on_time_escape(now);

        drop_maps
    }

    pub fn find_peer(&self, id: &DeviceId) -> Option<FoundPeer> {
        self.peers.lock().unwrap().find_peer(id, FindPeerReason::Other).map(|c| c.to_found_peer())
    }
}