use super::http_server::HttpServer;
use cyfs_base::{BuckyError, BuckyResult};

use async_std::net::{TcpListener, TcpStream};
use async_std::stream::StreamExt;
use async_std::task;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use futures::future::{AbortHandle, Abortable};

pub(super) struct HttpTcpListenerInner {
    listen: SocketAddr,

    server: HttpServer,

    // 用以停止
    abort_handle: Option<AbortHandle>,
}

impl HttpTcpListenerInner {
    pub fn new(listen: SocketAddr, server: HttpServer) -> Self {
        Self {
            listen,
            server,
            abort_handle: None,
        }
    }
}

#[derive(Clone)]
pub(super) struct HttpTcpListener(Arc<Mutex<HttpTcpListenerInner>>);


impl HttpTcpListener {
    pub fn new(addr: SocketAddr, server: HttpServer) -> Self {
        let inner = HttpTcpListenerInner::new(addr, server);

        Self(Arc::new(Mutex::new(inner)))
    }

    // 获取实际绑定的本地地址和端口
    pub fn get_local_addr(&self) -> SocketAddr {
        self.0.lock().unwrap().listen.clone()
    }

    pub async fn start(&self) -> BuckyResult<()> {
        let listen;
        {
            let listener = self.0.lock().unwrap();
            listen = listener.listen.clone();
        }

        let tcp_listener = TcpListener::bind(listen).await.map_err(|e| {
            let msg = format!(
                "tcp listener bind addr failed! addr={}, err={}",
                listen, e
            );
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        #[cfg(unix)]
        {
            use async_std::os::unix::io::AsRawFd;
            if let Err(e) = cyfs_util::set_socket_reuseaddr(tcp_listener.as_raw_fd()) {
                error!("set_socket_reuseaddr for {:?} error! err={}", listen, e);
            }
        }

        let local_addr = tcp_listener.local_addr().map_err(|e| {
            error!("get tcp listener local addr failed! {}", e);
            BuckyError::from(e)
        })?;

        let this = self.clone();

        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let future = Abortable::new(async move {
            let _r = this.run_inner(tcp_listener).await;
        }, abort_registration);

        // 更新本地的local addr
        // 保存abort_handle，用以后续stop
        {
            let mut listener = self.0.lock().unwrap();
            info!(
                "will update tcp listener local addr: {} -> {}",
                listener.listen, local_addr
            );
            listener.listen = local_addr.clone();

            assert!(listener.abort_handle.is_none());
            listener.abort_handle = Some(abort_handle);
        }

        task::spawn(future);

        Ok(())
    }

    pub fn stop(&self) {
        let mut listener = self.0.lock().unwrap();
        if let Some(abort_handle) = listener.abort_handle.take() {
            info!("will stop tcp listener! local addr={}", listener.listen);
            abort_handle.abort();
        } else {
            warn!("tcp listener not running or already stopped! local addr={}", listener.listen);
        }
    }

    async fn run_inner(
        &self,
        tcp_listener: TcpListener,
    ) -> BuckyResult<()> {
        let listen;
        let server;
        {
            let listener = self.0.lock().unwrap();

            listen = listener.listen.clone();
            server = listener.server.clone();
        }

        let addr = format!("http://{}", tcp_listener.local_addr().unwrap());
        info!("http listener at {}", addr);

        let mut incoming = tcp_listener.incoming();
        loop {
            match incoming.next().await {
                Some(Ok(tcp_stream)) => {
                    info!(
                        "tcp recv new connection from {:?}",
                        tcp_stream.peer_addr()
                    );

                    let addr = addr.clone();
                    let server = server.clone();
                    task::spawn(async move {
                        if let Err(e) = Self::accept(&server, addr, tcp_stream).await {
                            error!(
                                "tcp process http connection error: listen={} err={}",
                                listen, e
                            );
                        }
                    });
                }
                Some(Err(e)) => {
                    // FIXME 返回错误后如何处理？是否要停止
                    let listener = self.0.lock().unwrap();
                    error!(
                        "tcp recv http connection error! listener={}, err={}",
                        listener.listen, e
                    );
                }
                None => {
                    info!("tcp http finished! listen={}", listen);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn accept(
        server: &HttpServer,
        addr: String,
        stream: TcpStream,
    ) -> BuckyResult<()> {
        let peer_addr = stream.peer_addr()?;
        debug!(
            "starting accept new tcp connection at {} from {}",
            addr, &peer_addr
        );

        // 一条连接上只accept一次
        let opts = async_h1::ServerOptions::default();
        let ret = async_h1::accept_with_opts(stream, |mut req| async move {
            info!("recv tcp http request: {:?}, len={:?}", req, req.len());

            req.insert_header(cyfs_base::CYFS_REMOTE_DEVICE, peer_addr.ip().to_string());

            server.respond(req).await
        }, opts)
        .await;

        match ret {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("accept error, addr={}, peer={}, err={}", addr, peer_addr, e);

                // FIXME 一般是请求方直接断开导致的错误，是否需要判断并不再输出warn？
                //Err(BuckyError::from(e))
                Ok(())
            }
        }
    }
}
