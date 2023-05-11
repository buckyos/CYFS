use super::access_token::AccessTokenGen;
use super::controller::*;
use super::http_server::*;
use super::request::*;
use crate::OODControlMode;
use cyfs_base::*;
use cyfs_util::*;

use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone)]
pub enum ControlInterfaceAddrType {
    V4,
    V6,
    All,
}

#[derive(Debug, Clone)]
pub enum ControlTCPHost {
    Default(IpAddr),
    Strict(IpAddr),
}

#[derive(Debug)]
pub struct ControlInterfaceParam {
    pub mode: OODControlMode,
    pub tcp_port: Option<u16>,

    pub addr_type: ControlInterfaceAddrType,

    // Whether access is required, if it is provided, then it will try to bind the public ip
    pub require_access_token: bool,
    pub tcp_host: Option<ControlTCPHost>,
}

pub struct ControlInterface {
    tcp_listeners: Vec<HttpTcpListener>,
    access_info: ControlInterfaceAccessInfo,
}

impl ControlInterface {
    // tcp_port local port for tcp listening, pass None to use default value (depends on mode)
    pub fn new(param: ControlInterfaceParam, controller: &Controller) -> Self {
        let access_token = match param.require_access_token {
            true => Some(AccessTokenGen::new().gen_access_token(12)),
            false => None,
        };
        if access_token.is_some() {
            info!("will use access token: {}", access_token.as_ref().unwrap());
        }

        let (bind_addrs, display_addrs) = Self::get_tcp_hosts(&param);

        println!("will start ood control service at {:?}", bind_addrs);
        println!("will display control address as {:?}", display_addrs);


        let none_auth_server = Self::new_server(controller, None);
        let auth_server = if access_token.is_some() {
            Self::new_server(controller, access_token.clone())
        } else {
            none_auth_server.clone()
        };

        let mut tcp_listeners = Vec::new();
        for addr in &bind_addrs {
            let need_auth = match addr.ip() {
                IpAddr::V4(ip) => {
                    if ip.is_loopback() || ip.is_private() {
                        false
                    } else {
                        true
                    }
                }
                IpAddr::V6(_) => true,
            };

            if need_auth {
                tcp_listeners.push(HttpTcpListener::new(addr.clone(), auth_server.clone()));
            } else {
                tcp_listeners.push(HttpTcpListener::new(addr.clone(), none_auth_server.clone()));
            }
        }

        let access_info = ControlInterfaceAccessInfo {
            addrs: display_addrs,
            access_token,
        };

        controller.init_access_info(access_info.clone());

        Self {
            tcp_listeners,
            access_info,
        }
    }

    fn new_server(handler: &Controller, access_token: Option<String>) -> HttpServer {
        let mut server = HttpServer::new_server();
        HandlerEndpoint::register_server(handler, access_token, &mut server);

        HttpServer::new(server)
    }

    fn get_tcp_hosts(param: &ControlInterfaceParam) -> (Vec<SocketAddr>, Vec<SocketAddr>) {
        let port = param
            .tcp_port
            .clone()
            .unwrap_or(Self::default_port(param.mode));

        match param.tcp_host.clone() {
            Some(host) => match host {
                ControlTCPHost::Default(host) => {
                    let bind_host = if host.is_ipv6() {
                        IpAddr::V6(Ipv6Addr::UNSPECIFIED)
                    } else {
                        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
                    };

                    let bind_addr = SocketAddr::new(bind_host, port);
                    let addr = SocketAddr::new(host, port);
                    (vec![bind_addr], vec![addr])
                }
                ControlTCPHost::Strict(host) => {
                    let bind_addr = SocketAddr::new(host, port);
                    (vec![bind_addr.clone()], vec![bind_addr])
                }
            },
            None => {
                let mut addrs = Self::get_local_hosts();
                if param.require_access_token {
                    let mut public_hosts = Self::get_public_hosts();
                    addrs.append(&mut public_hosts);
                }

                let mut addrs: Vec<SocketAddr> = addrs
                    .into_iter()
                    .filter_map(|addr| match param.addr_type {
                        ControlInterfaceAddrType::V4 => {
                            if addr.is_ipv4() {
                                Some(addr)
                            } else {
                                None
                            }
                        }
                        ControlInterfaceAddrType::V6 => {
                            if addr.is_ipv6() {
                                Some(addr)
                            } else {
                                None
                            }
                        }
                        ControlInterfaceAddrType::All => Some(addr),
                    })
                    .collect();

                info!(
                    "got ood control tcp addrs: type={:?}, {:?}",
                    param.addr_type, addrs
                );

                addrs.iter_mut().for_each(|addr| {
                    addr.set_port(port);
                });
                (addrs.clone(), addrs)
            }
        }
    }

    fn default_port(mode: OODControlMode) -> u16 {
        match mode {
            OODControlMode::Daemon => cyfs_base::OOD_DAEMON_CONTROL_PORT,
            OODControlMode::Runtime => cyfs_base::CYFS_RUNTIME_DAEMON_CONTROL_PORT,
            OODControlMode::Installer => cyfs_base::OOD_INSTALLER_CONTROL_PORT,
            OODControlMode::App => {
                // For app, random ports are used (set to 0 to identify that random ports are used)
                0
            }
        }
    }

    fn get_local_hosts() -> Vec<SocketAddr> {
        // If we can't get the intranet ipv4, you can only use 127.0.0.1 loopback to avoid the security problem caused by binding the external port.
        match cyfs_util::get_system_hosts() {
            Ok(info) => {
                let mut private_ip_v4 = info.private_ip_v4;

                if private_ip_v4.is_empty() {
                    error!("retrieve system hosts but private ipv4 addrs not found!, now will use default");
                }
                private_ip_v4.push("127.0.0.1:0".parse().unwrap());
                private_ip_v4
            }
            Err(e) => {
                error!("retrieve system hosts failed, now will use default: {}", e);
                vec!["127.0.0.1:0".parse().unwrap()]
            }
        }
    }

    fn get_public_hosts() -> Vec<SocketAddr> {
        // Get the public network ipv4 and ipv6 addresses
        match cyfs_util::get_system_hosts() {
            Ok(mut info) => {
                let mut list = info.public_ip_v4;

                if list.is_empty() {
                    error!("retrieve system hosts but public ipv4 addrs not found!");
                    // public_ip_v4.push("0.0.0.0".to_string());
                }

                if info.ip_v6.len() > 0 {
                    list.append(&mut info.ip_v6);
                }

                list
            }
            Err(e) => {
                error!("retrieve system hosts failed, now will use default: {}", e);
                // vec!["0.0.0.0".to_string()]
                vec![]
            }
        }
    }

    pub fn get_access_info(&self) -> &ControlInterfaceAccessInfo {
        &self.access_info
    }

    pub async fn start(&self) -> BuckyResult<()> {
        // Local http control interface is enabled only in standard daemon and runtime mode
        let mut count = 0;
        for listener in &self.tcp_listeners {
            let ret = listener.start().await;
            if ret.is_ok() {
                count += 1;
            }
        }

        // The operation is considered to have failed only after all bindings have failed
        if count == 0 {
            let msg = format!("cyfs-control bind local address failed!");
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        Ok(())
    }

    pub fn stop(&self) {
        for listener in &self.tcp_listeners {
            listener.stop();
        }
    }

    // Get the local address and port of all tcp listeners
    pub fn get_tcp_addr_list(&self) -> Vec<SocketAddr> {
        self.tcp_listeners
            .iter()
            .map(|listener| listener.get_local_addr())
            .collect()
    }
}

#[cfg(test)]
mod test {
    use std::net::SocketAddr;

    #[test]
    fn test_fn() {
        let ip = "fe80::411c:ef94:73f1:8c17";
        let host = format!("{}:{}", ip, 100);
        let addr: SocketAddr = host.parse().unwrap();
        println!("{}", addr);
    }
}
