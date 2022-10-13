use std::{
    collections::{LinkedList, HashMap}, 
    net::{UdpSocket, SocketAddr}, 
    cell::RefCell,  
    thread, 
    time::Duration, 
};
use cyfs_debug::Mutex;
use async_std::{
    sync::{Arc}, 
    task, 
    future
};
use cyfs_base::*;
use crate::{
    types::*,  
    interface::udp::MTU
};
use std::time::{UNIX_EPOCH, SystemTime};

#[derive(Clone)]
pub struct Config {
    pub keepalive: Duration
}

#[derive(Clone, Debug)]
pub struct ProxyDeviceStub {
    pub id: DeviceId, 
    pub timestamp: Timestamp, 
}

#[derive(Clone, Debug)]
pub struct ProxyEndpointStub {
    endpoint: SocketAddr, 
    last_active: Timestamp
}

#[derive(Clone)]
struct ProxyTunnel {
    device_pair: (ProxyDeviceStub, ProxyDeviceStub), 
    endpoint_pair: (Option<ProxyEndpointStub>, Option<ProxyEndpointStub>), 
    last_active: Timestamp
}

impl std::fmt::Display for ProxyTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProxyTunnel")
    }
}

impl ProxyTunnel {
    fn new(device_pair: (ProxyDeviceStub, ProxyDeviceStub)) -> Self {
        Self {
            device_pair, 
            endpoint_pair: (None, None), 
            last_active: bucky_time_now()
        }
    }

    fn recyclable(&self, now: Timestamp, keepalive: Duration) -> bool {
        if now > self.last_active && Duration::from_micros(now - self.last_active) > keepalive {
            true
        } else {
            false
        }
    }

    fn on_device_pair(&mut self, device_pair: (ProxyDeviceStub, ProxyDeviceStub)) -> BuckyResult<()> {
        self.last_active = bucky_time_now();
        let (left, right) = device_pair;
        let (fl, fr) = {
            if left.id.eq(&self.device_pair.0.id) && right.id.eq(&self.device_pair.1.id) {
                Ok((&mut self.device_pair.0, &mut self.device_pair.1))
            } else if right.id.eq(&self.device_pair.0.id) && left.id.eq(&self.device_pair.1.id) {
                Ok((&mut self.device_pair.1, &mut self.device_pair.0))
            } else {
                trace!("{} ignore device pair ({:?}, {:?}) for not match {:?}", self, left, right, self.device_pair);
                Err(BuckyError::new(BuckyErrorCode::NotMatch, "device pair not match"))
            }
        }?;
        if left.timestamp > fl.timestamp {
            fl.timestamp = left.timestamp;
            self.endpoint_pair = (None, None);
            trace!("proxy tunnel update endpoint pair to (None, None)");
        }
        if right.timestamp > right.timestamp {
            fr.timestamp = right.timestamp;
            self.endpoint_pair = (None, None);
            trace!("proxy tunnel update endpoint pair to (None, None)");
        }
        Ok(())
    }

    fn on_proxied_datagram(&mut self, mix_hash: &KeyMixHash, from: &SocketAddr) -> Option<SocketAddr> {
        self.last_active = bucky_time_now();
        if self.endpoint_pair.0.is_none() {
            self.endpoint_pair.0 = Some(ProxyEndpointStub {
                endpoint: *from, 
                last_active: bucky_time_now()
            });
            trace!("{} mix_hash:{} update endpoint pair to {:?}", self, mix_hash, self.endpoint_pair);
            None
        } else if self.endpoint_pair.1.is_none() {
            let left = self.endpoint_pair.0.as_mut().unwrap(); 
            if left.endpoint.eq(from) {
                left.last_active = bucky_time_now();
            } else {
                self.endpoint_pair.1 = Some(ProxyEndpointStub {
                    endpoint: *from, 
                    last_active: bucky_time_now()
                });
            }
            trace!("{} mix_hash:{} update endpoint pair to {:?}", self, mix_hash, self.endpoint_pair);
            None
        } else {
            let left = self.endpoint_pair.0.as_mut().unwrap(); 
            let right = self.endpoint_pair.1.as_mut().unwrap(); 

            if left.endpoint.eq(from) {
                left.last_active = bucky_time_now();
                Some(right.endpoint)
            } else if right.endpoint.eq(from) {
                right.last_active = bucky_time_now();
                Some(left.endpoint)
            } else {
                *left = right.clone();
                right.endpoint = *from;
                right.last_active = bucky_time_now();
                trace!("ProxyTunnel mix_hash:{} mix_hash update endpoint pair to ({:?}, {:?})", mix_hash, left, right);
                Some(left.endpoint)
            }
        }
    }
}

#[derive(Clone)]
struct TunnelMixHash {
    tunnel: ProxyTunnel,
    mix_key: AesKey,
    mixhash: Vec<MixHashInfo>,
}

impl TunnelMixHash {
    pub fn recyclable(&self, now: Timestamp, keepalive: Duration) -> bool {
        self.tunnel.recyclable(now, keepalive)
    }

    pub fn new(mix_key: AesKey, tunnel: ProxyTunnel) -> Self {
        TunnelMixHash {
            tunnel,
            mix_key,
            mixhash: Vec::new(),
        }
    }

    pub fn rehash(&mut self, min: u64, max: u64) -> (Vec<KeyMixHash>, Vec<KeyMixHash>) {
        let mut timeout_n = 0;
        let mut next_ts = min;
        for h in self.mixhash.as_slice() {
            let t = h.minute_timestamp;
            if t < min {
                timeout_n += 1;
            } else if t > next_ts {
                next_ts = t + 1;
            }
        }

        let removed: Vec<MixHashInfo> = self.mixhash.splice(..timeout_n, vec![].iter().cloned()).collect();
        let removed = removed.iter().map(|h| h.hash.clone()).collect();

        let mut added = vec![];
        if next_ts < max {
            for t in next_ts..(max+1) {
                let h = MixHashInfo::new(self.mix_key.mix_hash(Some(t)), t);
                added.push(h.hash.clone());
                self.mixhash.push(h);
            }
        }

        (added, removed)
    }
}

#[derive(Clone)]
struct MixHashInfo {
    hash: KeyMixHash,
    minute_timestamp: u64,
}

impl MixHashInfo {
    pub fn new(hash: KeyMixHash, minute_timestamp: u64) -> Self {
        MixHashInfo {
            hash,
            minute_timestamp
        }
    }
}

struct TunnelsManager {
	tunnel_mixhash_map: HashMap<KeyMixHash, TunnelMixHash>,
    tunnel_mixkey_list: LinkedList<TunnelMixHash>,
    keepalive: Duration,
    mixhash_live_minutes: u64,
}

impl TunnelsManager {
    pub fn default() -> Self {
        let def_keepalive = 60;
        let def_mixhash_live_minute = 31;

        Self {
            tunnel_mixhash_map: HashMap::new(),
            tunnel_mixkey_list: LinkedList::new(),
            keepalive: Duration::from_secs(def_keepalive),
            mixhash_live_minutes: def_mixhash_live_minute,
        }
    }
}

impl TunnelsManager {
    fn minute_timestamp_range(&self) -> (u64, u64) {
        let minute_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 60;
        let min = minute_timestamp - (self.mixhash_live_minutes - 1) / 2;
        let max = minute_timestamp + (self.mixhash_live_minutes - 1) / 2;

        (min, max)
    }

    fn mixkey_update(&mut self,  mix_key: AesKey, device_pair: (ProxyDeviceStub, ProxyDeviceStub)) -> BuckyResult<()> {
        let tunnel = self.tunnel_mixhash_map.get(&mix_key.mix_hash(None)).unwrap();
        let mut tunnel = tunnel.tunnel.clone();
        let (left, right) = device_pair;

        let (fl, fr) = {
            if left.id.eq(&tunnel.device_pair.0.id) && right.id.eq(&tunnel.device_pair.1.id) {
                Ok((&mut tunnel.device_pair.0, &mut tunnel.device_pair.1))
            } else if right.id.eq(&tunnel.device_pair.0.id) && left.id.eq(&tunnel.device_pair.1.id) {
                Ok((&mut tunnel.device_pair.1, &mut tunnel.device_pair.0))
            } else {
                trace!("{} ignore device pair ({:?}, {:?}) for not match {:?}", tunnel, left, right, tunnel.device_pair);
                Err(BuckyError::new(BuckyErrorCode::NotMatch, "device pair not match"))
            }
        }?;
        if left.timestamp > fl.timestamp {
            fl.timestamp = left.timestamp;
            tunnel.endpoint_pair = (None, None);
            trace!("proxy tunnel update endpoint pair to (None, None)");
        }
        if right.timestamp > right.timestamp {
            fr.timestamp = right.timestamp;
            tunnel.endpoint_pair = (None, None);
            trace!("proxy tunnel update endpoint pair to (None, None)");
        }

        Ok(())
    }

    fn mixkey_add(&mut self,  mix_key: AesKey, device_pair: (ProxyDeviceStub, ProxyDeviceStub)) -> BuckyResult<()> {
        let mut tunnel = TunnelMixHash::new(mix_key.clone(), ProxyTunnel::new(device_pair));

        let (min, max) = self.minute_timestamp_range();
        let (added, _) = tunnel.rehash(min, max);

        self.tunnel_mixkey_list.push_front(tunnel.clone());

        for h in added.as_slice() {
            self.tunnel_mixhash_map.insert(h.clone(), tunnel.clone());
        }
        self.tunnel_mixhash_map.insert(mix_key.mix_hash(None), tunnel.clone());

        Ok(())
    }

    pub fn create_tunnel(&mut self, mix_key: AesKey, device_pair: (ProxyDeviceStub, ProxyDeviceStub)) -> BuckyResult<()> {
        let mix_hash = mix_key.mix_hash(None);

        if self.has_tunnel(&mix_hash) {
            self.mixkey_update(mix_key, device_pair)
        } else {
            self.mixkey_add(mix_key, device_pair)
        }
    }

    pub fn on_proxied_datagram(&mut self, datagram: &[u8], from: &SocketAddr) -> Option<SocketAddr> {
        match KeyMixHash::raw_decode(datagram) {
            Ok((mut mix_hash, _)) => {
                mix_hash.as_mut()[0] &= 0x7f;
                if let Some(tunnel) = self.tunnel_mixhash_map.get_mut(&mix_hash) {
                    trace!("{} recv datagram of mix_hash: {}", tunnel.tunnel, mix_hash);
                    tunnel.tunnel.on_proxied_datagram(&mix_hash, from)
                } else {
                    trace!("ignore datagram of mix_hash: {}", mix_hash);
                    None
                }
            }, 
            _ => {
                trace!("ignore datagram for invalid key foramt");
                None
            }
        }
    }

    pub fn has_tunnel(&self, mix_hash: &KeyMixHash) -> bool {
        if let Some(_) = self.tunnel_mixhash_map.get(mix_hash) {
            true
        } else {
            false
        }
    }

    pub fn rehash(&mut self) {
        let (min, max) = self.minute_timestamp_range();

        trace!("rehash min={} max={}", min, max);

        for (_, tunnel) in self.tunnel_mixkey_list.iter_mut().enumerate() {
            let (added, removed) = tunnel.rehash(min, max);
            for h in added.as_slice() {
                self.tunnel_mixhash_map.insert(h.clone(), tunnel.clone());
            }
            for h in removed.as_slice() {
                self.tunnel_mixhash_map.remove(h);
            }
        }
    }

    pub fn recycle(&mut self) {
        let now = bucky_time_now();

        trace!("recycle now={}", now);

        let mut removed = Vec::new();
        for (i, tunnel) in self.tunnel_mixkey_list.iter_mut().enumerate() {
            if tunnel.recyclable(now, self.keepalive) {
                removed.push(i-removed.len());
            }
        }

        for i in 0..removed.len() {
            let mut last_part = self.tunnel_mixkey_list.split_off(*removed.get(i).unwrap());
            let tunnel = last_part.pop_front().unwrap();
            self.tunnel_mixkey_list.append(&mut last_part);

            self.tunnel_mixhash_map.remove(&tunnel.mix_key.mix_hash(None));
            for i in 0..tunnel.mixhash.len() {
                let mixhash = tunnel.mixhash.get(i).unwrap();
                self.tunnel_mixhash_map.remove(&mixhash.hash);
            }
        }
    }
}

struct ProxyInterfaceImpl {
    config: Config, 
    socket: UdpSocket, 
    outer: SocketAddr, 
    tunnels: Mutex<TunnelsManager>,
}

#[derive(Clone)]
struct ProxyInterface(Arc<ProxyInterfaceImpl>);

impl std::fmt::Display for ProxyInterface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProxyInterface:{{endpoint:{:?}}}", self.local())
    }
}

thread_local! {
    static UDP_RECV_BUFFER: RefCell<[u8; MTU]> = RefCell::new([0u8; MTU]);
}

impl ProxyInterface {
    fn open(config: Config, local: SocketAddr, outer: Option<SocketAddr>) -> BuckyResult<Self> {
        let socket = UdpSocket::bind(local)
            .map_err(|e| {
                error!("ProxyInterface bind socket on {:?} failed for {}", local, e);
                e
            })?;
        let interface = Self(Arc::new(ProxyInterfaceImpl {
            config, 
            socket, 
            outer: outer.unwrap_or(local), 
            tunnels: Mutex::new(TunnelsManager::default()),
        }));

        let num_cpus = 4;
        let pool_size = num_cpus + 2;
        for _ in 0..pool_size {
            let interface = interface.clone();
            thread::spawn(move || {
                interface.proxy_loop();
            });
        }

        {
            let interface = interface.clone();
            task::spawn(async move {
                interface.timer().await;
            });
        }
        
        Ok(interface)
    }

    fn local(&self) -> SocketAddr {
        self.0.socket.local_addr().unwrap()
    }

    fn outer(&self) -> &SocketAddr {
        &self.0.outer
    }

    async fn timer(&self) {
        let tick_sec = 60;
        loop {
            {
                let mut tunnels = self.0.tunnels.lock().unwrap();
                tunnels.recycle();
                tunnels.rehash();
            }

            let _ = future::timeout(Duration::from_secs(tick_sec), future::pending::<()>()).await;
        }
    }

    fn proxy_loop(&self) {
        info!("{} started", self);
        loop {
            UDP_RECV_BUFFER.with(|thread_recv_buf| {
                let recv_buf = &mut thread_recv_buf.borrow_mut()[..];
                loop {
                    let rr = self.0.socket.recv_from(recv_buf);
                    if rr.is_ok() {
                        let (len, from) = rr.unwrap();
                        let recv = &recv_buf[..len];
                        trace!("{} recv datagram len {} from {:?}", self, len, from);
                        self.on_proxied_datagram(recv, &from);
                    } else {
                        let err = rr.err().unwrap();
                        if let Some(10054i32) = err.raw_os_error() {
                            // In Windows, if host A use UDP socket and call sendto() to send something to host B,
                            // but B doesn't bind any port so that B doesn't receive the message,
                            // and then host A call recvfrom() to receive some message,
                            // recvfrom() will failed, and WSAGetLastError() will return 10054.
                            // It's a bug of Windows.
                            trace!("{} socket recv failed for {}, ingore this error", self, err);
                        } else {
                            info!("{} socket recv failed for {}, break recv loop", self, err);
                            break;
                        }
                    }
                }
            });
        }
    }

    fn has_tunnel(&self, key: &KeyMixHash) -> bool {
        self.0.tunnels.lock().unwrap().has_tunnel(key)
    }

    fn on_proxied_datagram(&self, datagram: &[u8], from: &SocketAddr) {
        let proxy_to = {
            self.0.tunnels.lock().unwrap().on_proxied_datagram(datagram, from)
        };

        if let Some(proxy_to) = proxy_to {
            let _ = self.0.socket.send_to(datagram, &proxy_to);
        }
    }

    fn create_tunnel(&self, mix_key: AesKey, device_pair: (ProxyDeviceStub, ProxyDeviceStub)) -> BuckyResult<()> {
        self.0.tunnels.lock().unwrap().create_tunnel(mix_key, device_pair)
    }
}

pub struct ProxyTunnelManager {
    interface: ProxyInterface
}

impl std::fmt::Display for ProxyTunnelManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProxyTunnelManager")
    }
}

impl ProxyTunnelManager {
    pub fn open(config: Config, listen: &[(SocketAddr, Option<SocketAddr>)]) -> BuckyResult<Self> {
        //TODO: 支持多interface扩展
        let (local, outer) = listen[0];
        let interface = ProxyInterface::open(config, local, outer)?;
        Ok(Self {
            interface
        })
    }

    pub fn create_tunnel(&self, mix_key: &AesKey, device_pair: (ProxyDeviceStub, ProxyDeviceStub)) -> BuckyResult<SocketAddr> {
        let _ = self.interface.create_tunnel(mix_key.clone(), device_pair)?;
        Ok(self.interface.outer().clone())
    }

    pub fn tunnel_of(&self, key: &KeyMixHash) -> Option<SocketAddr> {
        self.interface.has_tunnel(key);
        Some(self.interface.outer().clone())
    }
}