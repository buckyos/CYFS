use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;
use log::*;
use crate::proxy::CyfsProxy;

#[derive(Clone)]
pub struct IpfsProxy(Arc<IpfsProxyInner>);

pub struct IpfsProxyInner {
    proxy: CyfsProxy,
    ipfs_gateway_port: u16,
    server: tide::Server<()>
}

impl IpfsProxy {
    pub(crate) fn new(proxy: CyfsProxy, ipfs_gateway_port: u16) -> Self {
        info!("create ipfs proxy, local gateway port {}", ipfs_gateway_port);
        Self(Arc::new(IpfsProxyInner{
            proxy,
            ipfs_gateway_port,
            server: tide::new()
        }))
    }

    fn is_ipns(path: &Path) -> bool {
        return path.starts_with("/ipns")
    }
}

impl IpfsProxyInner
{
    async fn call<State>(&self, req: tide::Request<State>) -> tide::Result
    where
        State: Clone + Send + Sync + 'static,
    {
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