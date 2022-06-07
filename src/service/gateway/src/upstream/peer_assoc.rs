use cyfs_base::{BuckyError, BuckyErrorCode, DeviceId};
use lazy_static::lazy_static;
use std::collections::{hash_map::Entry, HashMap};
use std::fmt;
use std::sync::Mutex;

pub struct PeerAssociationManager {
    udp_devices: HashMap<u16, DeviceId>,
    tcp_devices: HashMap<u16, DeviceId>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum AssociationProtocol {
    Tcp,
    Udp,
}

impl fmt::Display for AssociationProtocol {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let v = match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        };

        write!(fmt, "{}", v)
    }
}

impl AssociationProtocol {
    pub fn from(value: &str) -> Result<AssociationProtocol, BuckyError> {
        let ret = match value {
            "tcp" => AssociationProtocol::Tcp,
            "udp" => AssociationProtocol::Udp,
            v @ _ => {
                let msg = format!("invalid assoc protocol: {}", v);
                warn!("{}", msg);

                return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
            }
        };

        Ok(ret)
    }
}

impl PeerAssociationManager {
    pub fn new() -> Self {
        Self {
            udp_devices: HashMap::new(),
            tcp_devices: HashMap::new(),
        }
    }

    pub fn add(&mut self, protocol: AssociationProtocol, port: u16, device_id: DeviceId) {
        let container = match protocol {
            AssociationProtocol::Tcp => &mut self.tcp_devices,
            AssociationProtocol::Udp => &mut self.udp_devices,
        };

        match container.entry(port) {
            Entry::Vacant(v) => {
                info!("assoc remote device: {} {} <-> {}", protocol, port, device_id);
                v.insert(device_id);
            }
            Entry::Occupied(mut v) => {
                warn!(
                    "will replace old assoc: {} {} {} -> {}",
                    protocol,
                    port,
                    device_id,
                    v.get()
                );

                v.insert(device_id);
            }
        };
    }

    pub fn remove(&mut self, protocol: AssociationProtocol, port: u16) {
        let container = match protocol {
            AssociationProtocol::Tcp => &mut self.tcp_devices,
            AssociationProtocol::Udp => &mut self.udp_devices,
        };

        match container.remove(&port) {
            Some(device_id) => {
                info!(
                    "deassoc peerid with port: {} {} <-> {}",
                    protocol, port, device_id
                );
            }
            None => warn!("assoc peerid with port not exists: {}", port),
        };
    }

    pub fn query(&self, protocol: &AssociationProtocol, port: &u16) -> Option<DeviceId> {
        let container = match protocol {
            AssociationProtocol::Tcp => &self.tcp_devices,
            AssociationProtocol::Udp => &self.udp_devices,
        };

        container.get(port).map(|v| v.clone())
    }
}

lazy_static! {
    pub static ref PEER_ASSOC_MANAGER: Mutex<PeerAssociationManager> =
        Mutex::new(PeerAssociationManager::new());
}
