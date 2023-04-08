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

    server: Arc<tide::Server<()>>,

    // 用以停止
    abort_handle: Option<AbortHandle>,
}

impl HttpTcpListenerInner {
    pub fn new(listen: SocketAddr, server: Arc<tide::Server<()>>) -> Self {
        Self {
            listen,
            server,
            abort_handle: None,
        }
    }
}

#[derive(Clone)]
pub struct HttpTcpListener(Arc<Mutex<HttpTcpListenerInner>>);


impl HttpTcpListener {
    pub fn new(addr: SocketAddr, server: HttpServer) -> Self {
        Self::new_with_raw_server(addr, server.server().clone())
    }

    pub fn new_with_raw_server(addr: SocketAddr, server: Arc<tide::Server<()>>) -> Self {
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
            if let Err(e) = crate::set_socket_reuseaddr(tcp_listener.as_raw_fd()) {
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
        server: &Arc<tide::Server<()>>,
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

#[cfg(test)]
mod test {
    use super::*;
    use super::super::http_server::HttpServer;
    use async_std::net::*;
    use http_types::*;

    async fn test_utf8_header() {
        const UTF8_HEADER: &str = "错误Header";
        fn gen_utf8_header() -> http_types::headers::HeaderValue {
            let header = unsafe {
                http_types::headers::HeaderValue::from_bytes_unchecked(UTF8_HEADER.as_bytes().to_vec()) 
            };

            header
        }

        fn check_header(value: &http_types::headers::HeaderValue) {
            println!("test={}", value.as_str());

            let decoded_value = percent_encoding::percent_decode_str(value.as_str());
            let value = decoded_value.decode_utf8().unwrap();
            println!("origin test={}", value);
            assert_eq!(UTF8_HEADER, value);
        }

        async fn on_request(req: tide::Request<()>) -> tide::Result {
            let value = req.header("test").unwrap().last();
            check_header(value);

            let mut resp = tide::Response::new(StatusCode::Ok);
            
            resp.insert_header("test", gen_utf8_header());
            Ok(resp)
        }

        let mut server = tide::Server::new();
        server.at("/index.html").get(on_request);

        let server  = HttpServer::new(server);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1000);
        let server = HttpTcpListener::new(addr.clone(), server);
        server.start().await.unwrap();

        let tcp_stream = TcpStream::connect(addr).await.map_err(|e| {
            let msg = format!("connect to service error: {} {}", addr, e);
            println!("{}", msg);

            unreachable!();
        }).unwrap();

        let mut req = http_types::Request::new(Method::Get, "http://127.0.0.1/index.html");
        req.insert_header("test", gen_utf8_header());

        match async_h1::connect(tcp_stream, req).await {
            Ok(resp) => {
                println!("request to service success! {}, {:?}", addr, resp);
                let value = resp.header("test").unwrap().last();
                check_header(value);
            }
            Err(e) => {
                let msg = format!("request to service failed! {} {}", addr, e);
                println!("{}", msg);

                unreachable!();
            }
        }
    }

    #[test]
    fn test() {
        async_std::task::block_on(async move {
            test_utf8_header().await;
        })
    }
}