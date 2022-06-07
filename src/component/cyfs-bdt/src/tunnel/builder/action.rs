use std::time::{Duration};
use async_std::{sync::Arc, future, task};
use async_trait::{async_trait};
use cyfs_base::*;
use crate::protocol;
use super::super::*;
use tunnel::{Tunnel, TunnelState};
use log::*;


#[async_trait]
pub trait BuildTunnelAction: Send + Sync + std::fmt::Display {
    fn local(&self) -> &Endpoint;
    fn remote(&self) -> &Endpoint;
}

pub type DynBuildTunnelAction = Box<dyn BuildTunnelAction>;

struct SynUdpTunnelImpl {
    tunnel: udp::Tunnel, 
}

impl std::fmt::Display for SynUdpTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SynUdpTunnel{{tunnel:{}}}", self.0.tunnel)
    }
}

#[derive(Clone)]
pub struct SynUdpTunnel(Arc<SynUdpTunnelImpl>);
impl SynUdpTunnel {
    pub fn new(
        tunnel: udp::Tunnel, 
        first_box: Arc<protocol::PackageBox>, 
        interval: Duration) -> Self {
        let action = Self(Arc::new(SynUdpTunnelImpl {
            tunnel: tunnel.clone(), 
        }));
        // 有可能收到called之后触发 syn tunnel， 
        if tunnel.try_update_key(&first_box).is_err() {
            let action = action.clone();
            task::spawn(async move {
                loop {
                    match tunnel.state() {
                        TunnelState::Connecting => {
                            let result = tunnel.send_box(&first_box);
                            debug!("{} send first box result: {:?}", action, result);
                        },
                        _ => break,
                    } 
                    future::timeout(interval, future::pending::<()>()).await.err();
                }
            });
        }
        action
    }
}

#[async_trait]
impl BuildTunnelAction for SynUdpTunnel {
    fn local(&self) -> &Endpoint {
        self.0.tunnel.local()
    }

    fn remote(&self) -> &Endpoint {
        self.0.tunnel.remote()
    }
}



#[derive(Clone)]
pub struct ConnectTcpTunnel {
    tunnel: tcp::Tunnel
}

impl std::fmt::Display for ConnectTcpTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConnectTunnel{{tunnel:{}}}", self.tunnel)
    }
}


impl ConnectTcpTunnel {
    pub fn new(tunnel: tcp::Tunnel) -> Self {
        let _ = tunnel.connect();
        Self {
            tunnel
        }
    }
}

impl BuildTunnelAction for ConnectTcpTunnel {
    fn local(&self) -> &Endpoint {
        self.tunnel.local()
    }

    fn remote(&self) -> &Endpoint {
        self.tunnel.remote()
    }
}



