use log::*;
use std::{
    fmt, 
    time::Duration, 
    collections::{BTreeMap, LinkedList}, 
    sync::{Arc, RwLock}
};
use async_std::{
    future, 
    task
};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::{*, udp::{self, OnUdpPackageBox, OnUdpRawData}, tcp::{OnTcpInterface}}, 
    sn::client::PingClientCalledEvent, 
    stack::{Stack, WeakStack}
};
use super::container::{TunnelGuard, TunnelContainer, Config};

struct TunnelKeeper {
    reserving: Option<Timestamp>, 
    tunnel: TunnelGuard
}

impl TunnelKeeper {
    fn get(&self) -> TunnelGuard {
        self.tunnel.clone()
    }

    fn check(&mut self, when: Timestamp) -> bool {
        if self.tunnel.ref_count() > 1 {
            self.reserving = None;
            true
        } else if let Some(expire_at) = self.reserving {
            if when > expire_at {
                info!("{} expired at {}", &*self.tunnel, expire_at);
                false
            } else {
                true
            }
        } else {
            self.reserving = Some(when + self.tunnel.config().retain_timeout.as_micros() as u64);
            true
        }
    }
}

struct TunnelManagerImpl {
    stack: WeakStack, 
    entries: RwLock<BTreeMap<DeviceId, TunnelKeeper>>
}

#[derive(Clone)]
pub struct TunnelManager(Arc<TunnelManagerImpl>);

impl TunnelManager {
    pub fn new(stack: WeakStack) -> Self {
        let manager = Self(Arc::new(TunnelManagerImpl {
            stack, 
            entries: RwLock::new(BTreeMap::new())
        }));

        {
            let manager = manager.clone();
            task::spawn(async move {
                loop {
                    manager.check_recycle(bucky_time_now());
                    let _ = future::timeout(Duration::from_secs(1), future::pending::<()>()).await;           
                }
            });
        }

        manager
    }

    fn check_recycle(&self, when: Timestamp) {
        let mut entries = self.0.entries.write().unwrap(); 
        let mut remove = LinkedList::new();

        for (remote, keeper) in entries.iter() {
            if !keeper.check(when) {
                remove.push_back(remote.clone());
            }
        }

        for remote in remove {
            info!("{} will remove tunnel for not used, channel={}", self, remote);
            entries.remove(&remote);
        }
    }

    fn config_for(&self, _remote_const: &DeviceDesc) -> Config {
        // FIXME: 特化对不同remote的 tunnel config
        let stack = Stack::from(&self.0.stack);
        stack.config().tunnel.clone()
    }

    pub(crate) fn create_container(&self, remote_const: &DeviceDesc) -> Result<TunnelGuard, BuckyError> {
        let remote = remote_const.device_id();
        debug!("{} create new tunnel container of remote {}", self, remote);
        let mut entries = self.0.entries.write().unwrap();
        if let Some(tunnel) = entries.get(&remote) {
            Ok(tunnel.get())
        } else {
            let tunnel = TunnelGuard::new(TunnelContainer::new(self.0.stack.clone(), remote_const.clone(), self.config_for(remote_const)));
            entries.insert(remote, TunnelKeeper { reserving: None, tunnel: tunnel.clone() });
            Ok(tunnel)
        } 
    }

    pub(crate) fn container_of(&self, remote: &DeviceId) -> Option<TunnelGuard> {
        let entries = self.0.entries.read().unwrap();
        entries.get(&remote).map(|tunnel| {
            tunnel.get()
        })
    }

    pub(crate) fn reset(&self) {
        let entries = self.0.entries.read().unwrap();
        for (_, tunnel) in entries.iter() {
            tunnel.get().reset();
        }
    }

    pub(crate) fn on_statistic(&self) -> String {
        let tunnel_count = self.0.entries.read().unwrap().len();
        format!("TunnelCount: {}", tunnel_count)
    }
}

impl fmt::Display for TunnelManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TunnelManager{{local:{}}}", Stack::from(&self.0.stack).local_device_id())
    }
}

impl OnUdpPackageBox for TunnelManager {
    fn on_udp_package_box(&self, package_box: udp::UdpPackageBox) -> Result<(), BuckyError> {
        trace!("{} on_udp_package_box from remote {}", self, package_box.as_ref().remote());
        if let Some(tunnel) = self.container_of(package_box.as_ref().remote()) {
            tunnel.on_udp_package_box(package_box)
        } else {
            let first_package = &package_box.as_ref().packages_no_exchange()[0];
            if first_package.cmd_code() == PackageCmdCode::SynTunnel {
                let syn_tunnel: &SynTunnel = first_package.as_ref();
                // if syn_tunnel.sequence.is_valid(bucky_time_now()) {
                    let tunnel = self.create_container(syn_tunnel.from_device_desc.desc())?;
                    tunnel.on_udp_package_box(package_box)
                // } else {
                    // debug!("{} ignore udp package box from remote:{}, for syn tunnel seq timeout", self, package_box.as_ref().remote());
                    // Err(BuckyError::new(BuckyErrorCode::Timeout, "syn tunnel timeout"))
                // }
            } else {
                debug!("{} ignore udp package box from remote:{}, for first package is {:?}", self, package_box.as_ref().remote(), first_package.cmd_code());
                //FIXME: 支持从非syn tunnel包创建
                Err(BuckyError::new(BuckyErrorCode::InvalidInput, "tunnel's first package shoud be SynTunnel"))
            }
        }
    }
}

impl OnUdpRawData<(udp::Interface,DeviceId, MixAesKey, Endpoint)> for TunnelManager {
    fn on_udp_raw_data(&self, data: &[u8], context: (udp::Interface, DeviceId, MixAesKey, Endpoint)) -> Result<(), BuckyError> {
        trace!("{} on_udp_raw_data from remote {}", self, context.1);
        if let Some(tunnel) = self.container_of(&context.1) {
            tunnel.on_udp_raw_data(data, context)
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidInput, "tunnel's first package shoud be SynTunnel"))
        }
    }
}

impl OnTcpInterface for TunnelManager {
    fn on_tcp_interface(&self, interface: tcp::AcceptInterface, first_box: PackageBox) -> Result<OnPackageResult, BuckyError> {
        //全部转给tunnel container
        if let Some(tunnel) = self.container_of(first_box.remote()) {
            tunnel.on_tcp_interface(interface, first_box)
        } else {
            let first_package = &first_box.packages_no_exchange()[0];
            if first_package.cmd_code() == PackageCmdCode::SynTunnel {
                let syn_tunnel: &SynTunnel = first_package.as_ref();
                let tunnel = self.create_container(syn_tunnel.from_device_desc.desc())?;
                tunnel.on_tcp_interface(interface, first_box)
            } else if first_package.cmd_code() == PackageCmdCode::TcpSynConnection {
                let syn_tcp_stream: &TcpSynConnection = first_package.as_ref();
                let tunnel = self.create_container(syn_tcp_stream.from_device_desc.desc())?;
                tunnel.on_tcp_interface(interface, first_box)
            } else {
                Err(BuckyError::new(BuckyErrorCode::InvalidInput, "tunnel's tcp interface's first package shoud be SynTunnel or TcpSynConnection"))
            }
        }
    }
}

impl PingClientCalledEvent<PackageBox> for TunnelManager {
    fn on_called(&self, called: &SnCalled, caller_box: PackageBox) -> Result<(), BuckyError> {
        debug!("{} on_called from remote {} sequence {:?}", self, called.peer_info.desc().device_id(), called.seq);
        let first_package = &caller_box.packages_no_exchange()[0];
        if first_package.cmd_code() != PackageCmdCode::SynTunnel {
            debug!("{} ignore udp package box from remote:{}, for first package is {:?}", self, called.peer_info.desc().device_id(), first_package.cmd_code());
            return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "tunnel's first package shoud be SynTunnel"));
        }
        if let Some(tunnel) = self.container_of(caller_box.remote()) {
            tunnel.on_called(called, caller_box)
        } else {
            let syn_tunnel: &SynTunnel = first_package.as_ref();
            let tunnel = self.create_container(syn_tunnel.from_device_desc.desc())?;
            tunnel.on_called(called, caller_box)
        }
    }
}
