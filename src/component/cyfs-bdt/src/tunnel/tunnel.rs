use std::any::Any;
use async_trait::{async_trait};
use cyfs_base::*;
use crate::{types::*, protocol};

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum ProxyType {
    None, 
    Active(DeviceId), 
    Passive(DeviceId)
}

impl ProxyType {
    pub fn device_id(&self) -> Option<&DeviceId> {
        match self {
            Self::None => None, 
            Self::Active(device_id) => Some(device_id), 
            Self::Passive(device_id) => Some(device_id)
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum TunnelState {
    Connecting, 
    Active(Timestamp), 
    Dead, 
}

#[async_trait]
pub trait Tunnel: Send + Sync + std::fmt::Display {
    fn as_any(&self) -> &dyn Any;
    fn local(&self) -> &Endpoint;
    fn remote(&self) -> &Endpoint;
    fn state(&self) -> TunnelState; 
    fn proxy(&self) -> ProxyType;
    fn send_package(&self, packages: protocol::DynamicPackage) -> Result<usize, BuckyError>;
    fn raw_data_header_len(&self) -> usize;
    fn send_raw_data(&self, data: &mut [u8]) -> Result<usize, BuckyError>;
    fn ptr_eq(&self, other: &DynamicTunnel) -> bool;
    fn retain_keeper(&self);
    fn release_keeper(&self);
    fn mark_dead(&self, former_state: TunnelState);
    fn reset(&self);
    fn mtu(&self) -> usize;
}


pub struct DynamicTunnel(Box<dyn Tunnel>);
impl DynamicTunnel {
    pub fn new<T: 'static + Tunnel>(tunnel: T) -> Self {
        Self(Box::new(tunnel))
    }

    pub fn clone_as_tunnel<T: 'static + Tunnel + Clone>(&self) -> T {
        self.0.as_any().downcast_ref::<T>().unwrap().clone()
    }

    pub fn mtu(&self) -> usize {
        self.0.mtu()
    }
}

impl Clone for DynamicTunnel {
    fn clone(&self) -> Self {
        use super::udp;
        use super::tcp;

        if self.as_ref().local().is_udp() {
            Self::new(self.clone_as_tunnel::<udp::Tunnel>())
        } else if self.as_ref().local().is_tcp() {
            Self::new(self.clone_as_tunnel::<tcp::Tunnel>())
        } else {
            unreachable!()
        }
    }
}

impl AsRef<Box<dyn Tunnel>> for DynamicTunnel {
    fn as_ref(&self) -> &Box<dyn Tunnel> {
        &self.0
    } 
}


pub trait TunnelOwner: Send + Sync {
    fn sync_tunnel_state(&self, tunnel: &DynamicTunnel, former_state: TunnelState, new_state: TunnelState);
    fn clone_as_tunnel_owner(&self) -> Box<dyn TunnelOwner>;
}

