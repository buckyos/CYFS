use cyfs_base::*;

#[derive(Clone)]
pub struct BdtStackParams {
    pub device: Device,
    pub tcp_port_mapping: Vec<(Endpoint, u16)>,
    pub secret: PrivateKey,
    pub known_sn: Vec<Device>,
    pub known_device: Vec<Device>,
    pub known_passive_pn: Vec<Device>,
    pub udp_sn_only: Option<bool>,
}