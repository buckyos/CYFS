use super::http_server::{HttpRequestSource, HttpServerHandlerRef};
use super::ObjectListener;
use cyfs_base::{BuckyError, BuckyResult};
use cyfs_lib::RequestProtocol;
use cyfs_bdt::{StackGuard, StreamGuard as BdtStream, StreamListenerGuard};

use async_std::stream::StreamExt;
use async_std::task;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;

pub(super) struct ObjectHttpBdtListenerImpl {
    bdt_stack: StackGuard,
    vport: u16,
    listen: String,

    server: HttpServerHandlerRef,
}

impl ObjectHttpBdtListenerImpl {
    pub fn new(bdt_stack: StackGuard, vport: u16, server: HttpServerHandlerRef) -> Self {
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
pub(super) struct ObjectHttpBdtListener(Arc<ObjectHttpBdtListenerImpl>);

#[async_trait]
impl ObjectListener for ObjectHttpBdtListener {
    fn get_protocol(&self) -> RequestProtocol {
        RequestProtocol::HttpBdt
    }

    fn get_addr(&self) -> SocketAddr {
        unimplemented!();
    }

    async fn start(&self) -> BuckyResult<()> {
        Self::start(&self).await
    }

    async fn stop(&self) -> BuckyResult<()> {
        unreachable!();
    }

    async fn restart(&self) -> BuckyResult<()> {
        // TODO bdt应该只需要底层重启socket即可
        // should reset in bdt_stack's level
        Ok(())
    }
}

impl ObjectHttpBdtListener {
    pub fn new(bdt_stack: StackGuard, vport: u16, server: HttpServerHandlerRef) -> Self {
        let inner = ObjectHttpBdtListenerImpl::new(bdt_stack, vport, server);
        Self(Arc::new(inner))
    }

    async fn start(&self) -> BuckyResult<()> {
        // assert!(self.server.is_none());

        let bdt_listener = self.0.bdt_stack.stream_manager().listen(self.0.vport);
        if let Err(e) = bdt_listener {
            error!("interface bdt listen error! listen={} {}", self.0.listen, e);
            return Err(e);
        } else {
            info!("interface bdt listen: listen={}", self.0.listen);
        }

        let bdt_listener = bdt_listener.unwrap();

        let this = self.clone();
        task::spawn(async move {
            let _ = this.run(bdt_listener).await;
        });

        Ok(())
    }

    async fn run(&self, bdt_listener: StreamListenerGuard) -> BuckyResult<()> {
        // assert!(self.server.is_none());


        let mut incoming = bdt_listener.incoming();
        loop {
            match incoming.next().await {
                Some(Ok(pre_stream)) => {
                    // bdt内部一定会有info级别的日志，所以这个改为debug级别
                    debug!(
                        "bdt http recv new connection: listen={} remote={:?}, seq={:?}",
                        self.0.listen,
                        pre_stream.stream.remote(),
                        pre_stream.stream.sequence(),
                    );

                    // FIXME 暂时打印一下引用计数用以诊断错误
                    // pre_stream.stream.display_ref_count();

                    let this = self.clone();
                    task::spawn(async move {
                        if let Err(_e) = this.accept(pre_stream.stream).await {
                            /*
                            error!(
                                "interface process bdt stream error: addr={} err={}",
                                addr, e
                            );
                            */
                        }
                    });
                }
                Some(Err(e)) => {
                    // FIXME 返回错误后如何处理？是否要停止
                    error!(
                        "bdt http recv connection error! listen={}, err={}",
                        self.0.listen, e
                    );
                }
                None => {
                    info!("bdt http finished! listen={}", self.0.listen);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn accept(
        &self,
        stream: BdtStream,
    ) -> BuckyResult<()> {
        let seq = stream.sequence();
        let remote_addr = stream.remote();
        let remote_addr = (remote_addr.0.to_owned(), remote_addr.1);

        debug!(
            "bdt http service starting accept new connection at {} from {:?}, seq={:?}",
            self.0.listen, remote_addr, seq,
        );

        if let Err(e) = stream.confirm(&vec![]).await {
            error!(
                "bdt stream confirm error! remote={:?}, seq={:?}, {}",
                remote_addr, seq, e
            );

            return Err(e);
        }

        let begin = std::time::Instant::now();
        let device_id = remote_addr.0.to_string();
        let device_id_str = device_id.as_str();
        let remote_addr_ref = &remote_addr;
    
        let opts = async_h1::ServerOptions::default();
        let ret = async_h1::accept_with_opts(stream, |mut req| async move {
            info!(
                "recv bdt http request: source={}, seq={:?}, method={}, url={}, len={:?}",
                device_id_str,
                seq,
                req.method(),
                req.url(),
                req.len()
            );

            // req插入cyfs-device-id头部
            // 注意这里用insert而不是append，防止用户自带此header导致错误peerid被结算攻击
            req.insert_header(cyfs_base::CYFS_REMOTE_DEVICE, device_id_str.to_owned());

            let source = HttpRequestSource::Remote(remote_addr_ref.to_owned());
            match self.0.server.respond(source, req).await {
                Ok(resp) => {
                    let during = begin.elapsed().as_millis();
                    let status = resp.status();
                    if status.is_success() {
                        if during < 1000 {
                            debug!(
                                "bdt http request complete! seq={:?}, during={}ms",
                                seq, during,
                            );
                        } else {
                            info!(
                                "bdt http request complete! seq={:?}, during={}ms",
                                seq, during,
                            );
                        }
                    } else {
                        warn!(
                            "bdt http request complete with error! status={}, seq={:?}, during={}ms",
                            status, seq, during,
                        );
                    }
                    
                    Ok(resp)
                }
                Err(e) => {
                    error!(
                        "bdt http request error! seq={:?}, during={}, {}ms",
                        seq,
                        begin.elapsed().as_millis(),
                        e
                    );
                    Err(e)
                }
            }
        }, opts)
        .await;

        if let Err(e) = ret {
            error!(
                "bdt http accept error, err={}, addr={}, remote={:?}, seq={:?}, during={}",
                e, self.0.listen, remote_addr, seq, begin.elapsed().as_millis(),
            );
            return Err(BuckyError::from(e));
        }

        Ok(())
    }
}
