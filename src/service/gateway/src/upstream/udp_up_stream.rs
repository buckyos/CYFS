use super::{
    AssociationProtocol, UpstreamDatagramSender, DEFAULT_TIMEOUT, MAXIMUM_UDP_PAYLOAD_SIZE,
    PEER_ASSOC_MANAGER,
};
use cyfs_base::{BuckyError, DeviceId};

use async_std::net::UdpSocket;
use async_std::task;

use futures::future::{AbortHandle, Aborted};
use futures::prelude::*;
use lru_time_cache::LruCache;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct UdpUpStream {
    pub src_address: String,
    pub proxy_pass: (String, u16),
    pub sock: Arc<UdpSocket>,

    canceler: Option<AbortHandle>,

    // 来源是不是bdt协议
    remote_device_id: Option<DeviceId>,
}

impl Drop for UdpUpStream {
    fn drop(&mut self) {
        self.stop();
    }
}

impl UdpUpStream {
    /*
    pub fn new(src_address: &SocketAddr, proxy_pass: &(String, u16)) -> UdpUpStream {

        UdpUpStream {
            src_address: src_address.to_owned(),
            proxy_pass: proxy_pass.to_owned(),
            sock: None,
        }
    }
    */

    pub async fn new(
        src_address: String,
        proxy_pass: &(String, u16),
        remote_device_id: Option<&DeviceId>,
    ) -> Result<UdpUpStream, BuckyError> {
        let dest = Self::create_udp_socket(proxy_pass).await?;

        Ok(UdpUpStream {
            src_address,
            proxy_pass: proxy_pass.to_owned(),
            sock: Arc::new(dest),
            canceler: None,
            remote_device_id: remote_device_id.map(|v| v.clone()),
        })
    }

    async fn create_udp_socket(address: &(String, u16)) -> Result<UdpSocket, BuckyError> {
        let local_addr = "127.0.0.1:0";
        let socket = match UdpSocket::bind(&local_addr).await {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "init upstream udp socket error! addr={}, err={}",
                    local_addr, e
                );

                return Err(BuckyError::from(e));
            }
        };

        let remote_addr = format!("{}:{}", address.0, address.1);
        if let Err(e) = socket.connect(remote_addr).await {
            error!(
                "connect upstream udp socket error! addr={}:{}, err={}",
                address.0, address.1, e
            );

            return Err(BuckyError::from(e));
        }

        if let Err(e) = cyfs_util::init_udp_socket(&socket) {
            error!(
                "init udp proxy_pass socket error! {} -> {:?}, err={}",
                socket.local_addr().unwrap().to_string(),
                address,
                e
            );
        }

        Ok(socket)
    }

    pub fn stop(&self) {
        info!(
            "will stop udp upstream! src={}, proxy_pass={:?}",
            self.src_address, self.proxy_pass
        );

        if self.canceler.is_some() {
            let handle = self.canceler.as_ref().unwrap();
            handle.abort();
        }
    }

    pub async fn bind(
        &mut self,
        src: &Box<dyn UpstreamDatagramSender>,
        mut manager: UdpUpStreamManager,
    ) -> Result<(), BuckyError> {
        let dest = self.sock.clone();
        let src_address = self.src_address.clone();

        let proxy_pass_str = format!("{}:{}", self.proxy_pass.0, self.proxy_pass.1);

        // 对于bdt协议，建立peerid和upstream port的关联
        let port;
        if self.remote_device_id.is_some() {
            port = dest.local_addr().unwrap().port();
            PEER_ASSOC_MANAGER.lock().unwrap().add(
                AssociationProtocol::Udp,
                port,
                self.remote_device_id.as_ref().unwrap().clone(),
            );
        } else {
            port = 0u16;
        }

        let src = (*src).clone_sender();
        let (future, handle) = future::abortable(async move {
            let mut buf = vec![0u8; MAXIMUM_UDP_PAYLOAD_SIZE];
            let key = src_address.to_string();

            loop {
                match dest.recv(&mut buf).await {
                    Ok(recv_len) => {
                        if !manager.keep_alive(&key) {
                            info!("udp assocation breaked, {} -> {}", key, proxy_pass_str);
                            break;
                        }

                        if recv_len == 0 {
                            warn!(
                                "recv 0 byte package from upstream: {} -> {}",
                                proxy_pass_str, key
                            );
                            continue;
                        }

                        debug!(
                            "recv udp reply: {} -> {}, len={}",
                            proxy_pass_str, src_address, recv_len
                        );

                        let pkg_buf = &buf[..recv_len];

                        if let Err(e) = src.send_to(&pkg_buf, &src_address).await {
                            warn!(
                                "reply package to src_address error! {} -> {}, err={}",
                                proxy_pass_str, src_address, e
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            "recv from udp upstream error! proxy_pass={}, sock={}, err={}",
                            proxy_pass_str,
                            dest.local_addr().unwrap(),
                            e
                        );

                        // FIXME 接收出错后如何处理？
                    }
                }
            }
        });

        // 保存handle用以后续的取消
        self.canceler = Some(handle);

        let proxy_pass_str = format!("{}:{}", self.proxy_pass.0, self.proxy_pass.1);
        let src_address = self.src_address.clone();

        task::spawn(async move {
            match future.await {
                Ok(_) => {
                    info!(
                        "udp forward assocation complete, src={}, proxy_pass={:?}",
                        src_address, proxy_pass_str
                    );
                }
                Err(Aborted) => {
                    info!(
                        "udp forward assocation breaked, src={}, proxy_pass={:?}",
                        src_address, proxy_pass_str
                    );
                }
            };

            // 解除peerid和port的关联
            if port > 0 {
                PEER_ASSOC_MANAGER
                    .lock()
                    .unwrap()
                    .remove(AssociationProtocol::Udp, port);
            }
        });

        Ok(())
    }
}

#[derive(Clone)]
pub struct UdpUpStreamManager {
    list: Arc<Mutex<LruCache<String, UdpUpStream>>>,
    src_addr: String,
    proxy_pass: String,
    monitor_canceler: Option<AbortHandle>,
}

impl Drop for UdpUpStreamManager {
    fn drop(&mut self) {
        self.stop();
    }
}

impl UdpUpStreamManager {
    pub fn new(src_addr: &str, proxy_pass: &str) -> Self {
        let list = LruCache::with_expiry_duration(DEFAULT_TIMEOUT);

        Self {
            list: Arc::new(Mutex::new(list)),
            src_addr: src_addr.to_owned(),
            proxy_pass: proxy_pass.to_owned(),
            monitor_canceler: None,
        }
    }

    pub fn start(&mut self) {
        self.monitor();
    }

    pub fn stop(&mut self) {
        if let Some(canceler) = self.monitor_canceler.take() {
            info!("will stop udp upstream manager monitor: {}, {}", self.src_addr, self.proxy_pass);
            canceler.abort();
        }

        self.list.lock().unwrap().clear();
    }

    pub fn clone(&self) -> Self {
        Self {
            list: self.list.clone(),
            src_addr: self.src_addr.clone(),
            proxy_pass: self.proxy_pass.clone(),
            monitor_canceler: self.monitor_canceler.clone(),
        }
    }

    pub fn keep_alive(&mut self, key: &str) -> bool {
        let mut assoc = self.list.lock().unwrap();
        assoc.get(key).is_some()
    }

    // 监控所有的端口映射，移除超时的映射并关闭对应的socket(drop里面会stop)
    fn monitor(&mut self) {
        let list = self.list.clone();
        let proxy_pass = self.proxy_pass.clone();
        let src_addr = self.src_addr.clone();

        let (release_task, handle) = future::abortable(async move {
            let mut interval = async_std::stream::interval(Duration::from_secs(15));
            while let Some(_) = interval.next().await {
                let mut m = list.lock().unwrap();

                // 直接清除过期的元素，不能迭代这些元素，否则会导致这些元素被更新时间戳
                let _ = m.iter();

                if m.len() > 0 {
                    info!(
                        "udp associations alive count={}, {} <-> {}",
                        m.len(),
                        src_addr,
                        proxy_pass,
                    );
                }
            }
        });

        assert!(self.monitor_canceler.is_none());
        self.monitor_canceler = Some(handle);

        let proxy_pass = self.proxy_pass.clone();
        let src_addr = self.src_addr.clone();

        task::spawn(async move {
            match release_task.await {
                Ok(_) => {
                    info!(
                        "udp forward assocation monitor complete, src={}, proxy_pass={:?}",
                        src_addr, proxy_pass
                    );
                }
                Err(Aborted) => {
                    info!(
                        "udp forward assocation monitor cancelled, src={}, proxy_pass={:?}",
                        src_addr, proxy_pass
                    );
                }
            }
        });
    }

    pub async fn pick_stream(
        &mut self,
        addr: &str,
        proxy_pass: &(String, u16),
        sender: &Box<dyn UpstreamDatagramSender>,
        remote_device_id: Option<&DeviceId>,
    ) -> Result<Arc<UdpSocket>, BuckyError> {
        {
            let mut list = self.list.lock().unwrap();
            if let Some(v) = list.get(addr) {
                return Ok(v.sock.clone());
            }
        }

        info!("will assoc socket {} <---> {:?}", addr, proxy_pass);

        let mut stream = UdpUpStream::new(addr.to_owned(), proxy_pass, remote_device_id).await?;
        stream.bind(sender, self.clone()).await?;

        let ret = stream.sock.clone();

        {
            let mut list = self.list.lock().unwrap();
            list.insert(addr.to_owned(), stream);
        }

        Ok(ret)
    }
}
