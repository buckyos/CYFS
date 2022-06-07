use super::request::*;

use std::path::Path;
use sysinfo::{DiskExt, DiskType, ProcessorExt, SystemExt};

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
        // 获取第一个网卡地址
        #[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
        let mac_address = match mac_address::get_mac_address() {
            Ok(Some(v)) => v.to_string(),
            Ok(None) => {
                error!("get mac address but not found!");
                "".to_owned()
            }
            Err(e) => {
                error!("get mac address error! {}", e);
                "".to_owned()
            }
        };

        #[cfg(any(target_os = "android", target_os = "ios"))]
        let mac_address = "123".to_owned();

        let system = sysinfo::System::new_all();
        let processor = system.global_processor_info();
        let processor_brand = processor.brand();
        let memory = system.total_memory();

        let sn_path = Path::new("/etc/sn");
        let mut sn = String::from("");
        if sn_path.exists() {
            match std::fs::read_to_string(sn_path) {
                Ok(v) => sn = v,
                Err(e) => {
                    error!("load sn file error! file={}, {}", sn_path.display(), e);
                }
            }
        }

        let mut ssd_total_disk_space = 0 as u64;
        let mut ssd_available_disk_space = 0 as u64;
        let mut hdd_total_disk_space = 0 as u64;
        let mut hdd_available_disk_space = 0 as u64;
        let disk_list = system.disks();
        for disk in disk_list {
            match disk.type_() {
                // 移动设备和未知设备归入hdd
                DiskType::HDD | DiskType::Unknown(_) => {
                    hdd_total_disk_space += disk.total_space();
                    hdd_available_disk_space += disk.available_space();
                }
                DiskType::SSD => {
                    ssd_total_disk_space += disk.total_space();
                    ssd_available_disk_space += disk.available_space();
                }
            }
        }

        // 本地ip地址
        let private_ip_address: Vec<String> = match cyfs_util::get_system_hosts() {
            Ok(addr_list) => addr_list
                .private_ip_v4
                .iter()
                .map(|v| v.ip().to_string())
                .collect(),
            Err(_e) => Vec::new(),
        };

        DeviceInfo {
            mac_address,
            model: "Bucky OOD type storage".to_owned(),
            device_sn: sn,
            processor_brand: processor_brand.to_string(),
            total_memory: memory,
            ssd_total_disk_space,
            ssd_available_disk_space,
            hdd_total_disk_space,
            hdd_available_disk_space,
            private_ip_address,
        }
    }
}
