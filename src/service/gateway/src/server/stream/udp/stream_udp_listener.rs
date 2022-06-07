use crate::upstream::UdpUpStreamManager;
use crate::upstream::{UpstreamDatagramSender, MAXIMUM_UDP_PAYLOAD_SIZE};
use cyfs_base::BuckyError;

use async_std::net::UdpSocket;
use async_std::task;
use futures::future::{self, AbortHandle, Aborted};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

pub struct StreamUdpListener {
    pub listen: SocketAddr,
    proxy_pass: (String, u16),

    pub running: bool,
    canceler: Option<AbortHandle>,
}

impl StreamUdpListener {
    pub fn new(listen: SocketAddr) -> StreamUdpListener {
        StreamUdpListener {
            listen,
            proxy_pass: ("".to_owned(), 0),

            running: false,
            canceler: None,
        }
    }

    pub fn bind_proxy_pass(&mut self, proxy_pass: &(String, u16)) {
        assert!(proxy_pass.0.len() > 0);
        assert!(proxy_pass.1 > 0);

        self.proxy_pass = proxy_pass.clone();
    }

    pub fn stop(listener: &Arc<Mutex<StreamUdpListener>>) {
        let mut listener = listener.lock().unwrap();

        if let Some(abort) = listener.canceler.take() {
            info!("will stop udp datagram server {}", listener.listen);
            abort.abort();
        }

        listener.running = false;
    }

    pub async fn run(listener: Arc<Mutex<StreamUdpListener>>) -> Result<(), BuckyError> {
        {
            let mut listener = listener.lock().unwrap();

            // 这里判断一次状态
            if listener.running {
                warn!(
                    "stream udp listener already running! listen={}",
                    listener.listen
                );
                return Ok(());
            }

            // 标记为运行状态
            listener.running = true;
        }

        let ret = Self::run_inner(listener.clone()).await;

        listener.lock().unwrap().running = false;

        ret
    }

    async fn run_inner(udp_listener: Arc<Mutex<StreamUdpListener>>) -> Result<(), BuckyError> {
        let addr;
        let proxy_pass;
        {
            let listener = udp_listener.lock().unwrap();

            addr = listener.listen.clone();
            proxy_pass = listener.proxy_pass.clone();
        }

        let listener = match UdpSocket::bind(&addr).await {
            Ok(v) => {
                info!("stream udp listen at {}", addr);

                if let Err(e) = cyfs_util::init_udp_socket(&v) {
                    error!(
                        "init udp socket error! addr={}, proxy_pass={:?}, err={}",
                        addr, proxy_pass, e
                    );
                }

                v
            }
            Err(e) => {
                let msg = format!(
                    "udp stream server bind addr error! addr={}, err={}",
                    addr, e
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        let receiver = Arc::new(listener);
        let sender = receiver.clone_sender();

        let proxy_pass_str = format!("{}:{}", proxy_pass.0, proxy_pass.1);
        let addr2 = addr.clone();

        let mut upstream_manager = UdpUpStreamManager::new(&addr.to_string(), &proxy_pass_str);
        upstream_manager.start();

        let (future, handle) = future::abortable(async move {
            // 开始接收udp包并分发
            let mut buf = vec![0u8; MAXIMUM_UDP_PAYLOAD_SIZE];
            loop {
                let ret = receiver.recv_from(&mut buf).await;

                match ret {
                    Ok((recv_len, src_addr)) => {
                        // trace!("recv new udp datagram from {}, len={}", src_addr, recv_len);

                        if recv_len == 0 {
                            // windows上会是一个ICMP Port Unreachable Message
                            warn!("recv 0 byte package: {} -> {}", src_addr, addr);
                            continue;
                        }

                        trace!(
                            "recv udp package: {} -> {}, len={}",
                            src_addr, addr, recv_len
                        );

                        match upstream_manager
                            .pick_stream(&src_addr.to_string(), &proxy_pass, &sender, None)
                            .await
                        {
                            Ok(stream) => match stream.send(&buf[..recv_len]).await {
                                Ok(send_len) => {
                                    debug!(
                                        "forward udp package: {} -> {}, send_len={}",
                                        src_addr, proxy_pass_str, send_len
                                    );
                                    assert!(send_len == recv_len);
                                }
                                Err(e) => error!(
                                    "send package to upstream error! proxy_pass={}, err={}",
                                    proxy_pass_str, e
                                ),
                            },
                            Err(e) => {
                                error!(
                                    "pick udp upstram error! proxy_pass={:?}, src_addr={}, err={}",
                                    proxy_pass, src_addr, e
                                );
                            }
                        };
                    }
                    Err(e) => {
                        // FIXME 出错后是否要继续？
                        error!("recv udp package error: {:?}", e);
                    }
                }
            }
        });

        // 保存abort_handle
        {
            let mut listener = udp_listener.lock().unwrap();
            assert!(listener.canceler.is_none());
            listener.canceler = Some(handle);
        }

        match future.await {
            Ok(_) => {
                info!("udp datagram recv finished complete: {}", addr2);

                let mut listener = udp_listener.lock().unwrap();
                listener.canceler = None;
                listener.running = false;
            }
            Err(Aborted) => {
                info!("udp datagram recv aborted: {}", addr2);
            }
        };

        Ok(())
    }
}

pub struct StreamUdpListenerManager {
    server_list: Vec<Arc<Mutex<StreamUdpListener>>>,
}

impl StreamUdpListenerManager {
    pub fn new() -> StreamUdpListenerManager {
        StreamUdpListenerManager {
            server_list: Vec::new(),
        }
    }

    pub fn bind_proxy_pass(&mut self, proxy_pass: &(String, u16)) {
        for server in &self.server_list {
            let mut server = server.lock().unwrap();
            server.bind_proxy_pass(proxy_pass);
        }
    }

    /*
    {
        type: "bdt",
        stack: "bdt_public",
        vport: 80,
    }
    */
    pub fn load(&mut self, server_node: &toml::value::Table) -> Result<(), BuckyError> {
        let addr_list = match ::base::ListenerUtil::load_udp_listener(server_node) {
            Ok(v) => v,
            Err(e) => {
                return Err(e);
            }
        };

        for addr in addr_list {
            // 检查是否已经存在相同的stack+vport
            let ret = self.server_list.iter().any(|item| {
                let item = item.lock().unwrap();
                item.listen == addr
            });

            if ret {
                let msg = format!("tcp stream's tcp listener already exists! addr={}", addr);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }

            let server = StreamUdpListener::new(addr);
            let server = Arc::new(Mutex::new(server));
            self.server_list.push(server);
        }

        return Ok(());
    }

    pub fn start(&self) {
        for server in &self.server_list {
            let server = server.clone();
            if !server.lock().unwrap().running {
                task::spawn(async move {
                    let _r = StreamUdpListener::run(server).await;
                });
            }
        }
    }

    pub fn stop(&self) {
        for server in &self.server_list {
            StreamUdpListener::stop(server);
        }
    }
}
