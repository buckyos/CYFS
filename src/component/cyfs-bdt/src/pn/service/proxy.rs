use std::{
    collections::{BTreeMap, LinkedList}, 
    net::{UdpSocket, SocketAddr}, 
    cell::RefCell,  
    thread, 
    time::Duration, 
};
use cyfs_debug::Mutex;
use async_std::{
    sync::Arc, 
    task, 
    future
};
use cyfs_base::*;
use crate::{
    types::*,  
    interface::udp::MTU
};

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

    fn on_proxied_datagram(&mut self, key: &KeyMixHash, from: &SocketAddr) -> Option<SocketAddr> {
        self.last_active = bucky_time_now();
        if self.endpoint_pair.0.is_none() {
            self.endpoint_pair.0 = Some(ProxyEndpointStub {
                endpoint: *from, 
                last_active: bucky_time_now()
            });
            trace!("{} key:{} update endpoint pair to {:?}", self, key, self.endpoint_pair);
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
            trace!("{} key:{} update endpoint pair to {:?}", self, key, self.endpoint_pair);
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
                info!("ProxyTunnel key:{} key update endpoint pair to ({:?}, {:?})", key, left, right);
                Some(left.endpoint)
            }
        }
    }
}

struct ProxyInterfaceImpl {
    config: Config, 
    socket: UdpSocket, 
    outer: SocketAddr, 
    tunnels: Mutex<BTreeMap<KeyMixHash, ProxyTunnel>>
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
            tunnels: Mutex::new(BTreeMap::new())
        }));
        
        let pool_size = 4;
        for _ in 0..pool_size {
            let interface = interface.clone();
            thread::spawn(move || {
                interface.proxy_loop();
            });
        }

        {
            let interface = interface.clone();
            task::spawn(async move {
                interface.recycle_loop().await;
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

    async fn recycle_loop(&self) {
        loop {
            let now = bucky_time_now();
            {
                let mut to_remove = LinkedList::new();
                let mut tunnels = self.0.tunnels.lock().unwrap();
                for (key, tunnel) in tunnels.iter() {
                    if tunnel.recyclable(now, self.0.config.keepalive) {
                        to_remove.push_back(key.clone());
                    }
                }
                for key in to_remove {
                    info!("{} remove {}", self, key);
                    let _ = tunnels.remove(&key);
                }
            }
            let _ = future::timeout(Duration::from_secs(60), future::pending::<()>()).await;
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
        self.0.tunnels.lock().unwrap().contains_key(key)
    }

    fn on_proxied_datagram(&self, datagram: &[u8], from: &SocketAddr) {
        let proxy_to = match KeyMixHash::raw_decode(datagram) {
            Ok((mut key, _)) => {
                key.as_mut()[0] &= 0x7f;
                if let Some(tunnel) = self.0.tunnels.lock().unwrap().get_mut(&key) {
                    trace!("{} recv datagram of key: {}", self, key);
                    tunnel.on_proxied_datagram(&key, from)
                } else {
                    trace!("{} ignore datagram of key: {}", self, key);
                    None
                }
            }, 
            _ => {
                trace!("{} ignore datagram for invalid key foramt", self);
                None
            }
        };
        if let Some(proxy_to) = proxy_to {
            let _ = self.0.socket.send_to(datagram, &proxy_to);
        }
    }

    fn create_tunnel(&self, key: KeyMixHash, device_pair: (ProxyDeviceStub, ProxyDeviceStub)) -> BuckyResult<()> {
        let mut tunnels = self.0.tunnels.lock().unwrap();
        if let Some(tunnel) = tunnels.get_mut(&key) {
            info!("{} update tunnel key:{}, device pair:{:?}", self, key, device_pair);
            tunnel.on_device_pair(device_pair)
        } else {
            info!("{} create tunnel key:{}, device pair:{:?}", self, key, device_pair);
            tunnels.insert(key, ProxyTunnel::new(device_pair));
            Ok(())
        }
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

    pub fn create_tunnel(&self, key: KeyMixHash, device_pair: (ProxyDeviceStub, ProxyDeviceStub), _mix_key: &AesKey) -> BuckyResult<SocketAddr> {
        let _ = self.interface.create_tunnel(key, device_pair)?;
        Ok(self.interface.outer().clone())
    }

    pub fn tunnel_of(&self, key: &KeyMixHash) -> Option<SocketAddr> {
        self.interface.has_tunnel(key);
        Some(self.interface.outer().clone())
    }
}