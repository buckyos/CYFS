use super::http_server::HttpServer;
use cyfs_base::{BuckyError, BuckyResult};
use cyfs_bdt::{StackGuard, StreamGuard as BdtStream, StreamListenerGuard};

use async_std::stream::StreamExt;
use async_std::task;
use std::sync::{Arc, Mutex};

pub(super) struct HttpBdtListenerImpl {
    bdt_stack: StackGuard,
    vport: u16,
    listen: String,

    server: HttpServer,
}

impl HttpBdtListenerImpl {
    pub fn get_listen(&self) -> &str {
        &self.listen
    }

    pub fn new(bdt_stack: StackGuard, vport: u16, server: HttpServer) -> Self {
        let listen = format!("{}:{}", bdt_stack.local_device_id().to_string(), vport);
        Self {
            bdt_stack,
            vport,
            listen,
            server,
        }
    }
}

#[derive(Clone)]
pub(super) struct HttpBdtListener(Arc<Mutex<HttpBdtListenerImpl>>);


impl HttpBdtListener {
    pub fn new(bdt_stack: StackGuard, vport: u16, server: HttpServer) -> Self {
        let inner = HttpBdtListenerImpl::new(bdt_stack, vport, server);
        Self(Arc::new(Mutex::new(inner)))
    }

    async fn start(&self) -> BuckyResult<()> {
        // assert!(self.server.is_none());

        let stack;
        let vport;
        let listen;
        {
            let listener = self.0.lock().unwrap();
            stack = listener.bdt_stack.clone();
            vport = listener.vport;
            listen = listener.get_listen().to_owned();
        }

        let bdt_listener = stack.stream_manager().listen(vport);
        if let Err(e) = bdt_listener {
            error!("bdt listen error! listen={} {}", listen, e);
            return Err(e);
        } else {
            info!("bdt listen: listen={}", listen);
        }

        let bdt_listener = bdt_listener.unwrap();

        let this = self.clone();
        task::spawn(async move {
            let _r = this.run_inner(bdt_listener).await;
        });

        Ok(())
    }

    async fn run_inner(
        &self,
        bdt_listener: StreamListenerGuard,
    ) -> Result<(), BuckyError> {

        let server;
        let listen;
        {
            let listener = self.0.lock().unwrap();
            server = listener.server.clone();
            listen = listener.get_listen().to_owned();
        }

        let mut incoming = bdt_listener.incoming();
        loop {
            match incoming.next().await {
                Some(Ok(pre_stream)) => {
                    info!(
                        "bdt recv new connection: listen={} remote={:?}, sequence={:?}",
                        listen,
                        pre_stream.stream.remote(),
                        pre_stream.stream.sequence(),
                    );

                    let server = server.clone();
                    let addr = listen.clone();
                    task::spawn(async move {
                        if let Err(e) = Self::accept(&server, &addr, pre_stream.stream).await {
                            error!("bdt process stream error: addr={} err={}", addr, e);
                        }
                    });
                }
                Some(Err(e)) => {
                    // FIXME 返回错误后如何处理？是否要停止
                    error!(
                        "bdt http recv connection error! listen={}, err={}",
                        listen, e
                    );
                }
                None => {
                    info!("bdt http finished! listen={}", listen);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn accept(
        server: &HttpServer,
        addr: &str,
        stream: BdtStream,
    ) -> Result<(), BuckyError> {
        let remote_addr = stream.remote();
        let remote_addr = (remote_addr.0.to_owned(), remote_addr.1);
        debug!(
            "service starting accept new bdt connection at {} from {:?}",
            addr, remote_addr
        );

        if let Err(e) = stream.confirm(&vec![]).await {
            error!("bdt stream confirm error! {:?} {}", remote_addr, e);

            return Err(e);
        }

        let device_id = remote_addr.0.to_string();
        let device_id_str = device_id.as_str();
        let opts = async_h1::ServerOptions::default();
        let ret = async_h1::accept_with_opts(stream, |mut req| async move {
            info!("recv bdt http request: {:?}", req);

            // req插入cyfs-device-id头部
            // 注意这里用insert而不是append，防止用户自带此header导致错误peerid被结算攻击
            req.insert_header(cyfs_base::CYFS_REMOTE_DEVICE, device_id_str.to_owned());

            server.respond(req).await
        }, opts)
        .await;

        if let Err(e) = ret {
            error!(
                "accept error, err={}, addr={}, remote={:?}",
                e, addr, remote_addr
            );
            return Err(BuckyError::from(e));
        }

        Ok(())
    }
}
