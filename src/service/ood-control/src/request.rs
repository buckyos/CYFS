use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeviceInfo {
    pub mac_address: String,
    pub model: String,
    pub device_sn: String,
    pub processor_brand: String,
    pub total_memory: u64,
    pub ssd_total_disk_space: u64,
    pub ssd_available_disk_space: u64,
    pub hdd_total_disk_space: u64,
    pub hdd_available_disk_space: u64,
    pub private_ip_address: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BindInfo {
    pub area: String,
    pub owner_id: String,
    pub name: String,

    pub index: i32,
    pub unique_id: String,

    pub device_id: String,
    pub device: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CheckStatus {
    pub access_count: u32,
    pub last_access: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ControlInterfaceAccessInfo {
    pub addrs: Vec<SocketAddr>,
    pub access_token: Option<String>,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct CheckResponse {
    // 是否已经绑定了device.desc和device.sec
    pub activation: bool,

    pub check_status: HashMap<String, CheckStatus>,

    // 设备信息
    pub device_info: DeviceInfo,

    // ood-control访问信息
    pub access_info: ControlInterfaceAccessInfo,

    // 已经绑定的ood的device.desc
    pub bind_info: Option<BindInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActivateInfo {

    // 绑定的people
    pub owner: String,

    // 对应的device的索引
    pub index: i32,

    // device描述文件和密钥
    pub desc: String,
    pub sec: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActivateResult {
    pub result: u16,
    pub msg: String,
}
