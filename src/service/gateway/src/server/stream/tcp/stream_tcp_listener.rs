use crate::upstream::TcpUpStream;
use cyfs_base::BuckyError;
use cyfs_stack_loader::ListenerUtil;

use async_std::net::TcpListener;
use async_std::stream::StreamExt;
use async_std::task;
use futures::future::{self, AbortHandle, Aborted};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

pub struct StreamTcpListener {
    pub listen: SocketAddr,
    proxy_pass: (String, u16),

    pub running: bool,
    canceler: Option<AbortHandle>,
}

impl StreamTcpListener {
    pub fn new(listen: SocketAddr) -> StreamTcpListener {
        StreamTcpListener {
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

    pub fn stop(listener: &Arc<Mutex<StreamTcpListener>>) {
        let mut listener = listener.lock().unwrap();

        if let Some(abort) = listener.canceler.take() {
            info!("will stop bdt stream server {}", listener.listen);
            abort.abort();
        }

        listener.running = false;
    }

    pub async fn run(listener: Arc<Mutex<StreamTcpListener>>) -> Result<(), BuckyError> {
        {
            let mut listener = listener.lock().unwrap();

            // 这里判断一次状态
            if listener.running {
                warn!(
                    "stream tcp listener already running! listen={}",
                    listener.listen
                );
                return Ok(());
            }

            // 标记为运行状态
            listener.running = true;
            assert!(listener.canceler.is_none());
        }

        let ret = Self::run_inner(listener.clone()).await;

        listener.lock().unwrap().running = false;

        ret
    }

    async fn run_inner(tcp_listener: Arc<Mutex<StreamTcpListener>>) -> Result<(), BuckyError> {
        let addr;
        let proxy_pass;
        {
            let listener = tcp_listener.lock().unwrap();

            addr = listener.listen.clone();
            proxy_pass = listener.proxy_pass.clone();
        }

        let listener = match TcpListener::bind(&addr).await {
            Ok(v) => {
                info!("stream tcp listen at {}", addr);
                v
            }
            Err(e) => {
                let msg = format!(
                    "tcp stream server bind addr error! addr={}, err={}",
                    addr, e
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        #[cfg(unix)]
        {
            use async_std::os::unix::io::AsRawFd;
            if let Err(e) = cyfs_util::set_socket_reuseaddr(listener.as_raw_fd()) {
                error!("set_socket_reuseaddr for {:?} error! err={}", addr, e);
            }

            if let Err(e) = cyfs_util::set_socket_keepalive(listener.as_raw_fd()) {
                error!("set_socket_keepalive for {:?} error! err={}", addr, e);
            }
        }

        #[cfg(windows)]
        {
            use async_std::os::windows::io::AsRawSocket;

            if let Err(e) = cyfs_util::set_socket_keepalive(listener.as_raw_socket()) {
                error!("set_socket_keepalive for {:?} error! err={}", addr, e);
            }
        }

        let addr2 = addr.clone();

        let (future, handle) = future::abortable(async move {
            let mut incoming = listener.incoming();

            loop {
                let incoming_ret = incoming.next().await;
                let address = (proxy_pass.0.clone(), proxy_pass.1);

                match incoming_ret {
                    Some(v) => match v {
                        Ok(tcp_stream) => {
                            info!("recv new tcp connection from {:?}", tcp_stream.peer_addr());

                            // let address = address.clone();

                            task::spawn(async move {
                                let upstream = TcpUpStream::new(&address);

                                if let Err(e) = upstream.bind(tcp_stream).await {
                                    error!("bind tcp stream error: listen={}, err={}", addr2, e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("recv tcp connection error: listen={}, err={}", addr2, e);
                        }
                    },
                    None => {
                        info!("tcp stream incoming finished: listen={}", addr2);
                        break;
                    }
                }
            }
        });

        // 保存abort_handle
        {
            let mut listener = tcp_listener.lock().unwrap();
            assert!(listener.canceler.is_none());
            listener.canceler = Some(handle);
        }

        match future.await {
            Ok(_) => {
                info!("tcp stream recv incoming finished complete: {}", addr);

                let mut listener = tcp_listener.lock().unwrap();

                listener.canceler = None;
                listener.running = false;
            }
            Err(Aborted) => {
                info!("tcp stream recv incoming aborted: {}", addr);
            }
        };

        Ok(())
    }
}

pub struct StreamTcpListenerManager {
    server_list: Vec<Arc<Mutex<StreamTcpListener>>>,
}

impl StreamTcpListenerManager {
    pub fn new() -> StreamTcpListenerManager {
        StreamTcpListenerManager {
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
        let addr_list = match ListenerUtil::load_tcp_listener(server_node) {
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

            let server = StreamTcpListener::new(addr);
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
                    let _r = StreamTcpListener::run(server).await;
                });
            }
        }
    }

    pub fn stop(&self) {
        for server in &self.server_list {
            StreamTcpListener::stop(server);
        }
    }
}
