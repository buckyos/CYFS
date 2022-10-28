use std::path::Path;
use std::sync::{Arc, RwLock};
use async_trait::async_trait;
use log::*;
use cyfs_base::{BuckyError, BuckyErrorCode};
use crate::ipfs_stub::IPFSStub;
use crate::proxy::CyfsProxy;

#[derive(PartialEq)]
enum IpfsDaemonStatus {
    Disable,
    Running,
    NotInit,
    NotRunning,
    Starting
}

#[derive(Clone)]
pub struct IpfsProxy(Arc<IpfsProxyInner>);

pub struct IpfsProxyInner {
    proxy: CyfsProxy,
    ipfs_gateway_port: u16,
    stub: IPFSStub,

    status: RwLock<IpfsDaemonStatus>
}

impl IpfsProxy {
    pub(crate) fn new(proxy: CyfsProxy, ipfs_gateway_port: u16) -> Self {
        info!("create ipfs proxy, local gateway port {}", ipfs_gateway_port);
        let ipfs_data_dir = cyfs_util::get_service_data_dir("ipfs");
        let ipfs_prog = std::env::current_exe().unwrap().parent().unwrap().join("ipfs.exe");
        let status = if ipfs_prog.exists() {
            IpfsDaemonStatus::NotInit
        } else {
            warn!("ipfs daemon not found at {}, disable ipfs proxy", ipfs_prog.display());
            IpfsDaemonStatus::Disable
        };
        Self(Arc::new(IpfsProxyInner {
            proxy,
            ipfs_gateway_port,
            stub: IPFSStub::new(&ipfs_data_dir, &ipfs_prog),
            status: RwLock::new(status)
        }))
    }

    pub(crate) fn start_ipfs(&self) {
        info!("ipfs proxy start");
        if *self.0.status.read().unwrap() == IpfsDaemonStatus::Disable {
            return;
        }
        let inner = self.0.clone();
        async_std::task::spawn(async move {
            info!("ipfs proxy start in task");
            *inner.status.write().unwrap() = IpfsDaemonStatus::Starting;
            if !inner.stub.is_init().await {
                if let Err(e) = inner.stub.init(inner.ipfs_gateway_port, inner.ipfs_gateway_port+1, inner.ipfs_gateway_port+2).await {
                    error!("ipfs init err {}", e);
                    *inner.status.write().unwrap() = IpfsDaemonStatus::NotRunning;
                }
            }

            if inner.stub.start().await {
                *inner.status.write().unwrap() = IpfsDaemonStatus::Running;
            } else {
                *inner.status.write().unwrap() = IpfsDaemonStatus::NotRunning;
            }
        });
    }

    fn is_ipns(path: &Path) -> bool {
        return path.starts_with("/ipns")
    }
}

impl IpfsProxyInner
{
    fn check_status(&self) -> Result<(), tide::Error> {
        match *self.status.read().unwrap() {
            IpfsDaemonStatus::Running => Ok(()),
            IpfsDaemonStatus::Disable => Err(tide::Error::new(tide::StatusCode::BadGateway, BuckyError::new(BuckyErrorCode::UnSupport, "ipfs proxy disable"))),
            IpfsDaemonStatus::NotInit => Err(tide::Error::new(tide::StatusCode::BadGateway, BuckyError::new(BuckyErrorCode::NotInit, "ipfs proxy not inited"))),
            IpfsDaemonStatus::NotRunning => Err(tide::Error::new(tide::StatusCode::BadGateway, BuckyError::new(BuckyErrorCode::UnSupport, "ipfs proxy daemon start error"))),
            IpfsDaemonStatus::Starting => Err(tide::Error::new(tide::StatusCode::BadGateway, BuckyError::new(BuckyErrorCode::UnSupport, "ipfs proxy daemon starting, please wait"))),
        }
    }

    async fn call<State>(&self, req: tide::Request<State>) -> tide::Result
    where
        State: Clone + Send + Sync + 'static,
    {
        self.check_status()?;
        let url = req.url();
        let path = Path::new(url.path());
        if IpfsProxy::is_ipns(path) {
            info!("ipfs proxy recv ipns req path {}", path.display());
            let mut new_req: http_types::Request = req.into();
            new_req.url_mut().set_port(Some(self.ipfs_gateway_port)).unwrap();
            self.respond(new_req).await
        } else {
            info!("ipfs proxy recv req path {}, treat as ipfs path", path.display());
            let mut new_req: http_types::Request = req.into();
            new_req.url_mut().set_port(Some(self.ipfs_gateway_port)).unwrap();
            self.respond(new_req).await
        }
    }

    async fn respond(&self, req: http_types::Request) -> tide::Result {
        let address = format!("{}:{}", req.url().host_str().unwrap(), req.url().port().unwrap());

        let stream = async_std::net::TcpStream::connect(&address).await.map_err(|e|{
            error!("connect to {} err {}", &address, e);
            tide::Error::new(tide::StatusCode::InternalServerError, e)
        })?;

        async_h1::connect(stream, req).await.map(tide::Response::from_res)
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for IpfsProxy
    where
        State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: tide::Request<State>) -> tide::Result {
        self.0.call(req).await
    }
}