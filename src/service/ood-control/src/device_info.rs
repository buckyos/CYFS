use super::request::*;

pub(super) struct DeviceInfoGen;

impl DeviceInfoGen {
    pub fn default() -> DeviceInfo {
        DeviceInfo {
            mac_address: "".to_string(),
            model: "".to_string(),
            device_sn: "".to_string(),
            processor_brand: "".to_string(),
            total_memory: 0,
            ssd_total_disk_space: 0,
            ssd_available_disk_space: 0,
            hdd_total_disk_space: 0,
            hdd_available_disk_space: 0,
            private_ip_address: Vec::new(),
        }
    }

    pub fn new() -> DeviceInfo {
        let system_info = async_std::task::block_on(async move {
            cyfs_util::SYSTEM_INFO_MANAGER.get_system_info().await
        });

        // Local intranet ip address
        let private_ip_address: Vec<String> = match cyfs_util::get_system_hosts() {
            Ok(addr_list) => addr_list
                .private_ip_v4
                .iter()
                .map(|v| v.ip().to_string())
                .collect(),
            Err(_e) => Vec::new(),
        };

        DeviceInfo {
            mac_address: system_info.mac_address.unwrap_or("[Unknown]".to_owned()),
            model: "Bucky OOD type storage".to_owned(),
            device_sn: system_info.device_sn.unwrap_or("".to_owned()),
            processor_brand: system_info.cpu_brand,
            total_memory: system_info.total_memory,
            ssd_total_disk_space: system_info.ssd_disk_total,
            ssd_available_disk_space: system_info.ssd_disk_avail,
            hdd_total_disk_space: system_info.hdd_disk_total,
            hdd_available_disk_space: system_info.hdd_disk_avail,
            private_ip_address,
        }
    }
}
