use super::http_tcp_listener::*;
use cyfs_base::*;

use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum HttpInterfaceHost {
    // local loopback addr
    Local,

    // 0.0.0.0
    Unspecified,

    Specified(Vec<IpAddr>),
}

impl Default for HttpInterfaceHost {
    fn default() -> Self {
        Self::Local
    }
}

impl std::str::FromStr for HttpInterfaceHost {
    type Err = BuckyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "local" | "l" => Self::Local,
            "unspecified" | "u" => Self::Unspecified,
            _ => {
                let list: Vec<_> = s.split(",").collect();
                let mut addrs = vec![];
                for v in list {
                    let addr = IpAddr::from_str(v).map_err(|e| {
                        let msg = format!("invalid ip addr: {}, {}", v, e);
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::InvalidParam, msg)
                    })?;
                    addrs.push(addr);
                }

                Self::Specified(addrs)
            }
        };

        Ok(ret)
    }
}

pub struct HttpInterface {
    list: Vec<HttpTcpListener>,
}

impl HttpInterface {
    pub fn new(host: HttpInterfaceHost, port: u16, server: tide::Server<()>) -> Self {
        let server = Arc::new(server);
        let mut list = vec![];
        match host {
            HttpInterfaceHost::Local => {
                let addr = std::net::SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                    port,
                );

                let interface = HttpTcpListener::new_with_raw_server(addr, server);
                list.push(interface);
            }
            HttpInterfaceHost::Unspecified => {
                let addr = std::net::SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                    port,
                );

                let interface = HttpTcpListener::new_with_raw_server(addr, server);
                list.push(interface);
            }
            HttpInterfaceHost::Specified(addrs) => {
                for addr in addrs {
                    let addr = std::net::SocketAddr::new(addr, port);

                    let interface = HttpTcpListener::new_with_raw_server(addr, server.clone());
                    list.push(interface);
                }
            }
        }

        Self { list }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        for interface in &self.list {
            interface.start().await?;
        }

        Ok(())
    }
}
