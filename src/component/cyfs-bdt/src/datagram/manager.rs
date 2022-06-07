use std::{
    collections::{BTreeMap, btree_map::{self, Entry}}, 
    sync::{RwLock}, 
    time::Duration
};
use async_std::sync::Arc;
use rand::{thread_rng, Rng};
use cyfs_base::*;
use crate::{
    protocol::{Datagram, OnPackage, OnPackageResult}, 
    tunnel::TunnelContainer, 
    stack::{WeakStack, Stack}, 
};
use super::{
    tunnel::{DatagramTunnel, DatagramTunnelGuard}, 
};

pub enum ReservedVPort {
    Channel = 1,
    Dht = 2, 
    Debug = 3,
}

impl Into<u16> for ReservedVPort {
    fn into(self) -> u16 {
        self as u16
    }
}

const MIN_DATAGRAM_USER_VPORT: u16 = 1024;

#[derive(Clone)]
pub struct Config {
    pub min_random_vport: u16,
    pub max_random_vport: u16,
    pub max_try_random_vport_times: usize,
    pub piece_cache_duration: Duration,
    pub recv_cache_count: usize,
}

struct DatagramManagerImpl {
    stack: WeakStack,
    cfg: Config,
    tunnels: RwLock<BTreeMap<u16, DatagramTunnel>>
}

impl DatagramManagerImpl {
    fn new(stack: WeakStack) -> DatagramManagerImpl {
        let cfg = Stack::from(&stack).config().datagram.clone();
        DatagramManagerImpl { stack, cfg, tunnels: RwLock::new(Default::default()) }
    }

    pub(crate) fn bind(&self, vport: u16) -> Result<DatagramTunnelGuard, BuckyError> {
        let mut vport = vport;
        let mut tunnel_map = self.tunnels.write().unwrap();
        if vport == 0 {
            let mut rng = thread_rng();
            for _ in 0..self.cfg.max_try_random_vport_times {
                let try_vport = rng.gen_range(self.cfg.min_random_vport, self.cfg.max_random_vport);
                if tunnel_map.get(&try_vport).is_none() {
                    vport = try_vport;
                    break;
                }
            }

            if vport == 0 {
                log::warn!("datagram bind random-vport failed.");
                return Err(BuckyError::new(BuckyErrorCode::AddrInUse, "try random-vport failed"));
            }
        }

        match tunnel_map.entry(vport) {
            btree_map::Entry::Vacant(entry) => {
                let tunnel = DatagramTunnel::new(self.stack.clone(), vport, 100);
                entry.insert(tunnel.clone());
                Ok(DatagramTunnelGuard::from(tunnel))
            },

            Entry::Occupied(_) => Err(BuckyError::new(BuckyErrorCode::AddrInUse, "the vport is in-use"))
        }
    }

    fn unbind(&self, vport: u16) -> Option<DatagramTunnel> {
        self.tunnels.write().unwrap().remove(&vport)
    }

    fn find_tunnel(&self, vport: u16) -> Option<DatagramTunnel> {
        let tunnels = self.tunnels.read().unwrap();
        tunnels.get(&vport).map(|t| t.clone())
    }
}

#[derive(Clone)]
pub struct DatagramManager(Arc<DatagramManagerImpl>);

impl DatagramManager {
    pub fn new(stack: WeakStack) -> DatagramManager {
        DatagramManager {
            0: Arc::new(DatagramManagerImpl::new(stack))
        }
    }

    pub(crate) fn bind_reserved(&self, vport: ReservedVPort) -> Result<DatagramTunnelGuard, BuckyError> {
        self.0.bind(vport.into())
    }

    pub fn bind(&self, vport: u16) -> Result<DatagramTunnelGuard, BuckyError> {
        if vport != 0 && vport < MIN_DATAGRAM_USER_VPORT {
            log::warn!("datagram bind try use reserved vport({})", vport);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, "reserved-vport"));
        }
        self.0.bind(vport)
    }

    pub(super) fn unbind(&self, vport: u16) {
        if vport >= MIN_DATAGRAM_USER_VPORT {
            self.0.unbind(vport);
        }
    }

    pub fn stack(&self) -> Stack {
        Stack::from(&self.0.stack)
    }

    pub(crate) fn tunnel_closed(&self, tunnel: &DatagramTunnel) {
        let mut tunnels = self.0.tunnels.write().unwrap();
        let removed_tunnel = tunnels.remove(&tunnel.vport());
        match removed_tunnel {
            Some(t) => {
                log::info!("datagram tunnel({}) closed.", t.vport());
            },
            None => {
                log::warn!("datagram try remove tunnel({}) not exist, maybe is dup closed.", tunnel.vport())
            }
        }
    }
}

impl OnPackage<Datagram, &TunnelContainer> for DatagramManager {
    fn on_package(&self, pkg: &Datagram, from: &TunnelContainer) -> Result<OnPackageResult, BuckyError> {
        if let Some(tunnel) = self.0.find_tunnel(pkg.to_vport) {
            tunnel.on_package(pkg, from)
        } else {
            log::warn!("datagram recv data to unknown vport: {}, from: {}.", pkg.to_vport, from);
            Err(BuckyError::new(BuckyErrorCode::ErrorState, "no datagram-tunnel bind"))
        }
    }
}