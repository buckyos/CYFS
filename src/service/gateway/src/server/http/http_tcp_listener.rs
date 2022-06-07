use async_std::net::{TcpListener, TcpStream};
use async_std::stream::StreamExt;
use async_std::task;
use http_types::{Response, StatusCode};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use cyfs_base::BuckyError;
use futures::future::{self, AbortHandle, Aborted};

use super::http_listener_base::HttpListenerBase;
use base::ListenerUtil;

#[derive(Debug)]
pub(super) struct HttpTcpListener {
    pub listen: SocketAddr,
    base: Arc<Mutex<HttpListenerBase>>,

    running: bool,
    canceler: Option<AbortHandle>,
}

impl HttpTcpListener {
    pub fn new() -> HttpTcpListener {
        HttpTcpListener {
            listen: "0.0.0.0:0".parse().unwrap(),
            base: Arc::new(Mutex::new(HttpListenerBase::new())),

            running: false,
            canceler: None,
        }
    }

    pub fn bind_forward(&self, forward_id: u32) {
        info!(
            "http tcp listener bind new forward: listener={}, forward_id={}",
            self.listen, forward_id
        );

        let mut base = self.base.lock().unwrap();
        base.bind_forward(forward_id);
    }

    pub fn unbind_forward(&self, forward_id: u32) -> bool {
        let mut base = self.base.lock().unwrap();

        if base.unbind_forward(forward_id) {
            info!(
                "http tcp listener unbind forward: listener={}, forward_id={}",
                self.listen, forward_id
            );
            true
        } else {
            false
        }
    }

    pub fn load(
        &mut self,
        _server_node: &toml::value::Table,
    ) -> Result<(), BuckyError> {
        Ok(())
    }

    async fn run(listener: Arc<Mutex<HttpTcpListener>>) -> Result<(), BuckyError> {
        {
            let mut listener = listener.lock().unwrap();

            // 这里判断一次状态
            if listener.running {
                warn!(
                    "http tcp listener already running! listen={}",
                    listener.listen
                );
                return Ok(());
            }

            listener.running = true;
        }

        let ret = Self::run_inner(listener.clone()).await;

        listener.lock().unwrap().running = false;

        ret
    }

    async fn run_inner(listener: Arc<Mutex<HttpTcpListener>>) -> Result<(), BuckyError> {
        let listen;
        {
            let listener = listener.lock().unwrap();

            listen = listener.listen.clone();
        }

        let tcp_listener = match TcpListener::bind(listen).await {
            Ok(v) => v,
            Err(e) => {
                let msg = format!(
                    "http tcp listener bind addr failed! addr={}, err={}",
                    listen, e
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        #[cfg(unix)]
        {
            use async_std::os::unix::io::AsRawFd;
            if let Err(e) = cyfs_util::set_socket_reuseaddr(tcp_listener.as_raw_fd()) {
                error!("set_socket_reuseaddr for {:?} error! err={}", listen, e);
            }
        }

        let addr = format!("http://{}", tcp_listener.local_addr()?);
        let addr2 = addr.clone();
        let listener2 = listener.clone();

        let (future, handle) = future::abortable(async move {
            let mut incoming = tcp_listener.incoming();
            loop {
                let incoming_ret = incoming.next().await;
                match incoming_ret {
                    Some(Ok(tcp_stream)) => {
                        info!("recv new tcp connection from {:?}", tcp_stream.peer_addr());

                        let addr = addr.clone();
                        let listener = listener.clone();
                        task::spawn(async move {
                            if let Err(e) = Self::accept(&listener, addr, tcp_stream).await {
                                error!("process tcp http connection error: err={}", e);
                            }
                        });
                    }
                    Some(Err(e)) => {
                        // FIXME 返回错误后如何处理？是否要停止
                        let listener = listener.lock().unwrap();
                        error!(
                            "recv tcp http connection error! listener={}, err={}",
                            listener.listen, e
                        );
                    }
                    None => {
                        break;
                    }
                };
            }
        });

        // 保存abort_handle
        {
            let mut listener = listener2.lock().unwrap();
            assert!(listener.canceler.is_none());
            listener.canceler = Some(handle);
        }

        match future.await {
            Ok(_) => {
                info!(
                    "http tcp listener recv incoming finished complete: {}",
                    addr2
                );

                let mut listener = listener2.lock().unwrap();
                listener.canceler = None;
                listener.running = false;
            }
            Err(Aborted) => {
                info!("http tcp listener recv incoming aborted: {}", addr2);
            }
        };

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(abort) = self.canceler.take() {
            info!("will stop http tcp listener {}", self.listen);
            abort.abort();
        }

        self.running = false;
    }

    async fn accept(
        listener: &Arc<Mutex<HttpTcpListener>>,
        addr: String,
        stream: TcpStream,
    ) -> Result<(), BuckyError> {
        let peer_addr = stream.peer_addr()?;
        info!(
            "starting accept new tcp connection at {} from {}",
            addr, &peer_addr
        );

        // 一条连接上只accept一次
        let opts = async_h1::ServerOptions::default();
        let ret = async_h1::accept_with_opts(stream.clone(), |mut req| async move {
            info!("recv tcp http request: {:?}, len={:?}", req, req.len());

            let base;
            {
                let server = listener.lock().unwrap();
                if server.running {
                    base = server.base.clone();
                } else {
                    error!("tcp http server already closed, server={}", server.listen);
                    return Ok(Response::new(StatusCode::InternalServerError));
                }
            }

            // 用户自己的请求不可附带CYFS_REMOTE_PEER，避免被攻击
            req.remove_header(cyfs_base::CYFS_REMOTE_DEVICE);

            let resp = HttpListenerBase::dispatch_request(&base, req).await;
            Ok(resp)
        }, opts)
        .await;

        match ret {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("accept error, addr={}, peer={}, err={}", addr, peer_addr, e);

                // FIXME 一般是请求方直接断开导致的错误，是否需要判断并不再输出warn？
                // Err(BuckyError::from(e))
                Ok(())
            }
        }

        /*
        accept是否要响应stop事件
        match async_std::future::timeout(Duration::from_secs(5), accept_future).await {
                Ok(v) => match v {
                    Ok(_) => {
                        return Ok(())
                    },
                    Err(e) => {
                        error!("accept error, addr={}, peer={}, err={}", addr, peer_addr, e);
                        return Err(BuckyError::from(e));
                    }
                }
                Err(e) => {

                    // timeout，检查运行状态
                    let listener = listener.lock().unwrap();
                    if !listener.running {
                        info!("http tcp listener stopped! listener={}", listener.listen);
                        return Err(BuckyError::from(e));
                    }
                }
            }
        }
        */
    }
}

pub(super) struct HttpTcpListenerManager {
    server_list: Vec<Arc<Mutex<HttpTcpListener>>>,
}

impl HttpTcpListenerManager {
    pub fn new() -> HttpTcpListenerManager {
        HttpTcpListenerManager {
            server_list: Vec::new(),
        }
    }

    /*
    {
        type: "tcp",
        listen: "127.0.0.1:80",
    }
    */
    pub fn load(
        &mut self,
        server_node: &toml::value::Table,
        forward_id: u32,
    ) -> Result<(), BuckyError> {
        let listener_list: Vec<SocketAddr> = ListenerUtil::load_tcp_listener(server_node)?;
        assert!(listener_list.len() > 0);

        for listen in listener_list {
            let item = self.get_or_create(&listen);
            let mut item = item.lock().unwrap();

            // TODO 加载额外的选项
            if let Err(e) = item.load(server_node) {
                error!(
                    "load tcp listener failed! err={}, node={:?}",
                    e, server_node
                );
            }

            item.bind_forward(forward_id);
        }

        return Ok(());
    }

    fn get_or_create(&mut self, listen: &SocketAddr) -> Arc<Mutex<HttpTcpListener>> {
        let ret = self.server_list.iter().any(|item| {
            let item = item.lock().unwrap();
            item.listen == *listen
        });

        if !ret {
            let mut server = HttpTcpListener::new();
            server.listen = listen.clone();
            let server = Arc::new(Mutex::new(server));
            self.server_list.push(server);
        }

        return self.get_item(listen).unwrap();
    }

    pub fn get_item(&mut self, listen: &SocketAddr) -> Option<Arc<Mutex<HttpTcpListener>>> {
        for server in &self.server_list {
            if server.lock().unwrap().listen == *listen {
                return Some(server.clone());
            }
        }

        return None;
    }

    pub fn unbind_forward(&self, forward_id: u32) {
        for server in &self.server_list {
            let mut server = server.lock().unwrap();
            server.unbind_forward(forward_id);

            // 如果没有绑定任何转发器，那么停止该listener
            if server.base.lock().unwrap().forward_count() == 0 {
                server.stop();
            }
        }
    }

    // 启动所有服务
    pub fn start(&self) {
        for server in &self.server_list {
            if !server.lock().unwrap().running {
                let server = server.clone();
                task::spawn(async move {
                    let _r = HttpTcpListener::run(server).await;
                });
            }
        }
    }

    pub fn stop_idle(&self) {
        for server in &self.server_list {
            let mut server = server.lock().unwrap();
            if server.base.lock().unwrap().forward_count() == 0 {
                server.stop();
            }
        }
    }
}
