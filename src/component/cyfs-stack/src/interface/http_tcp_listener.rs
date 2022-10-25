use super::http_server::{HttpRequestSource, HttpServerHandlerRef};
use super::ObjectListener;
use cyfs_base::*;
use cyfs_lib::{BaseTcpListener, BaseTcpListenerHandler, RequestProtocol};

use async_std::net::TcpStream;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct ObjectHttpTcpListener {
    tcp_server: BaseTcpListener,

    listen_url: String,

    // 当前bdt协议栈的device_id
    device_id: DeviceId,

    server: HttpServerHandlerRef,
}

#[async_trait]
impl ObjectListener for ObjectHttpTcpListener {
    fn get_protocol(&self) -> RequestProtocol {
        RequestProtocol::HttpLocal
    }

    fn get_addr(&self) -> SocketAddr {
        self.tcp_server.get_addr()
    }

    async fn start(&self) -> BuckyResult<()> {
        self.tcp_server.start().await
    }

    async fn stop(&self) -> BuckyResult<()> {
        self.tcp_server.stop().await;
        Ok(())
    }

    async fn restart(&self) -> BuckyResult<()> {
        self.tcp_server.stop().await;
        self.tcp_server.start().await
    }
}

impl ObjectHttpTcpListener {
    pub fn new(addr: SocketAddr, device_id: DeviceId, server: HttpServerHandlerRef) -> Self {
        let listen_url = format!("http://{}", addr);
        let ret = Self {
            tcp_server: BaseTcpListener::new(addr),
            listen_url,
            device_id,
            server,
        };

        let tcp_handler = Arc::new(Box::new(ret.clone()) as Box<dyn BaseTcpListenerHandler>);
        ret.tcp_server.bind_handler(tcp_handler);

        ret
    }

    pub fn get_listen(&self) -> String {
        self.tcp_server.get_listen()
    }

    async fn accept(&self, stream: TcpStream) -> BuckyResult<()> {
        let peer_addr = stream.peer_addr()?;
        debug!(
            "starting accept new tcp connection at {} from {}",
            self.listen_url, &peer_addr
        );

        // 一条连接上只accept一次
        let begin = std::time::Instant::now();
        let opts = async_h1::ServerOptions::default();
        let ret = async_h1::accept_with_opts(
            stream,
            |mut req| async move {
                info!(
                    "recv tcp http request: url={}, method={}, len={:?}, peer={}",
                    req.url(),
                    req.method(),
                    req.len(),
                    peer_addr,
                );

                // http请求都是同机请求，需要设定为当前device
                req.insert_header(cyfs_base::CYFS_REMOTE_DEVICE, self.device_id.to_string());

                let source = HttpRequestSource::Local(peer_addr.clone());
                match self.server.respond(source, req).await {
                    Ok(resp) => {
                        let during = begin.elapsed().as_millis();
                        let status = resp.status();
                        if status.is_success() {
                            if during < 1000 {
                                debug!(
                                    "tcp http request complete! peer={}, during={}ms",
                                    peer_addr, during,
                                );
                            } else {
                                info!(
                                    "tcp http request complete! peer={}, during={}ms",
                                    peer_addr, during,
                                );
                            }
                        } else {
                            warn!(
                            "tcp http request complete with error! status={}, peer={}, during={}ms",
                            status, peer_addr, during,
                        );
                        }

                        Ok(resp)
                    }
                    Err(e) => {
                        error!(
                            "tcp http request error! peer={}, during={}, {}ms",
                            peer_addr,
                            begin.elapsed().as_millis(),
                            e
                        );
                        Err(e)
                    }
                }
            },
            opts,
        )
        .await;

        if let Err(e) = ret {
            error!(
                "tcp http accept error, err={}, addr={}, peer={}, during={}ms",
                e, self.listen_url, peer_addr, begin.elapsed().as_millis(),
            );
            // FIXME 一般是请求方直接断开导致的错误，是否需要判断并不再输出warn？
            //Err(BuckyError::from(e))
            Ok(())
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl BaseTcpListenerHandler for ObjectHttpTcpListener {
    async fn on_accept(&self, tcp_stream: TcpStream) -> BuckyResult<()> {
        if let Err(e) = self.accept(tcp_stream).await {
            error!(
                "object tcp process http connection error: listen={} err={}",
                self.listen_url, e
            );
        }

        Ok(())
    }
}
