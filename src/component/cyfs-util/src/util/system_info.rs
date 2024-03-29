use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{CpuExt, DiskExt, DiskType, NetworkExt, RefreshKind, System, SystemExt};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub name: String,

    // Device Serial Number
    pub device_sn: Option<String>,

    // How long the system has been running since it was last booted, in microseconds
    pub uptime: u64,

    // The time the system was last booted, in bucky time
    pub boot_time: u64,

    pub mac_address: Option<String>,

    pub cpu_brand: String,
    pub cpu_usage: f32,

    // memory size in bytes
    pub total_memory: u64,
    pub used_memory: u64,

    // Bytes transferred between each refresh cycle
    pub received_bytes: u64,
    pub transmitted_bytes: u64,

    // total bytes of all networks since last booted
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,

    // SSD drive capacity and available capacity, including Unknown, in bytes
    pub ssd_disk_total: u64,
    pub ssd_disk_avail: u64,

    // HDD capacity and available capacity, in bytes
    pub hdd_disk_total: u64,
    pub hdd_disk_avail: u64,
}

impl Default for SystemInfo {
    fn default() -> Self {
        let sys = System::new();
        let uptime = sys.uptime() * 1000 * 1000;
        let boot_time = cyfs_base::unix_time_to_bucky_time(sys.boot_time() * 1000 * 1000);

        Self {
            name: "".to_owned(),
            device_sn: None,
            uptime,
            boot_time,
            mac_address: None,
            cpu_brand: "".to_owned(),
            cpu_usage: 0.0,
            total_memory: 0,
            used_memory: 0,
            received_bytes: 0,
            transmitted_bytes: 0,
            total_received_bytes: 0,
            total_transmitted_bytes: 0,
            ssd_disk_total: 0,
            ssd_disk_avail: 0,
            hdd_disk_total: 0,
            hdd_disk_avail: 0,
        }
    }
}


impl SystemInfo {
    pub fn update(&mut self, updater: SystemInfoUpdater) {
        if let Some(value) = updater.name {
            self.name = value;
        }

        if let Some(value) = updater.device_sn {
            self.device_sn = Some(value);
        }

        if let Some(value) = updater.uptime {
            self.uptime = value;
        }

        if let Some(value) = updater.boot_time {
            self.boot_time = value;
        }

        if let Some(value) = updater.mac_address {
            self.mac_address = Some(value);
        }

        // cpu
        if let Some(value) = updater.cpu_brand {
            self.cpu_brand = value;
        }

        if let Some(value) = updater.cpu_usage {
            self.cpu_usage = value;
        }

        // memory
        if let Some(value) = updater.total_memory {
            self.total_memory = value;
        }

        if let Some(value) = updater.used_memory {
            self.used_memory = value;
        }

        // bytes trans
        if let Some(value) = updater.received_bytes {
            self.received_bytes = value;
        }

        if let Some(value) = updater.transmitted_bytes {
            self.transmitted_bytes = value;
        }

        if let Some(value) = updater.total_received_bytes {
            self.total_received_bytes = value;
        }

        if let Some(value) = updater.total_transmitted_bytes {
            self.total_transmitted_bytes = value;
        }

        // disk
        if let Some(value) = updater.ssd_disk_total {
            self.ssd_disk_total = value;
        }

        if let Some(value) = updater.ssd_disk_avail {
            self.ssd_disk_avail = value;
        }

        if let Some(value) = updater.hdd_disk_total {
            self.hdd_disk_total = value;
        }

        if let Some(value) = updater.hdd_disk_avail {
            self.hdd_disk_avail = value;
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemInfoUpdater {
    pub name: Option<String>,

    pub device_sn: Option<String>,

    pub uptime: Option<u64>,

    pub boot_time: Option<u64>,

    pub mac_address: Option<String>,

    pub cpu_brand: Option<String>,
    pub cpu_usage: Option<f32>,

    // memory size in bytes
    pub total_memory: Option<u64>,
    pub used_memory: Option<u64>,

    // Bytes transferred between each refresh cycle
    pub received_bytes: Option<u64>,
    pub transmitted_bytes: Option<u64>,

    // total bytes of all networks since last booted
    pub total_received_bytes: Option<u64>,
    pub total_transmitted_bytes: Option<u64>,

    // SSD drive capacity and available capacity, including Unknown, in bytes
    pub ssd_disk_total: Option<u64>,
    pub ssd_disk_avail: Option<u64>,

    // HDD capacity and available capacity, in bytes
    pub hdd_disk_total: Option<u64>,
    pub hdd_disk_avail: Option<u64>,
}

impl Default for SystemInfoUpdater {
    fn default() -> Self {
        Self {
            name: None,
            device_sn: None,
            uptime: None,
            boot_time: None,
            mac_address: None,
            cpu_brand: None,
            cpu_usage: None,
            total_memory: None,
            used_memory: None,
            received_bytes: None,
            transmitted_bytes: None,
            total_received_bytes: None,
            total_transmitted_bytes: None,
            ssd_disk_total: None,
            ssd_disk_avail: None,
            hdd_disk_total: None,
            hdd_disk_avail: None,
        }
    }
}

struct SystemInfoManagerInner {
    running: bool,
    last_access_time: Instant,
    max_idle_time: Duration,

    info_inner: SystemInfo,
    handler: System,

    external_updater: Option<SystemInfoUpdater>,
}

impl SystemInfoManagerInner {
    pub fn new() -> Self {
        let r = RefreshKind::new()
            .with_networks()
            .with_networks_list()
            .with_memory()
            .with_cpu(sysinfo::CpuRefreshKind::new().with_cpu_usage())
            .with_disks()
            .with_disks_list();
        let handler = System::new_with_specifics(r);

        let mut info_inner = SystemInfo::default();

        let s = System::new();
        info_inner.name = match s.host_name() {
            Some(name) => {
                let trim = '\0';
                if name.ends_with(trim) {
                    name[..name.len() - 1].to_owned()
                } else {
                    name
                }
            }
            None => "MY PC".to_owned(),
        };

        info!("os name: {:?}", info_inner.name);

        Self {
            running: false,

            last_access_time: Instant::now(),
            max_idle_time: Duration::from_secs(15),

            info_inner,
            handler,

            external_updater: None,
        }
    }

    pub fn update_system_info(&mut self, updater: SystemInfoUpdater) {
        info!("system update from external: {:?}", updater);

        self.info_inner.update(updater.clone());

        self.external_updater = Some(updater);
    }

    pub fn check_idle(&mut self) {
        let now = Instant::now();
        if now - self.last_access_time >= self.max_idle_time {
            info!(
                "system info extend max idle duration, now will stop: last_access={:?}",
                self.last_access_time
            );
            self.running = false;
        }
    }

    pub fn refresh(&mut self) {
        self.handler.refresh_all();
        self.udpate_sn();
        self.update_memory();
        self.update_cpu();
        self.update_mac();
        self.update_network();
        self.update_disks();

        if let Some(updater) = self.external_updater.clone() {
            self.info_inner.update(updater);
        }
    }

    fn udpate_sn(&mut self) {
        let sn_path = std::path::Path::new("/etc/sn");
        let device_sn = if sn_path.exists() {
            match std::fs::read_to_string(sn_path) {
                Ok(v) => Some(v),
                Err(e) => {
                    error!("load sn file error! file={}, {}", sn_path.display(), e);
                    None
                }
            }
        } else {
            None
        };

        self.info_inner.device_sn = device_sn;
    }

    fn update_memory(&mut self) {
        self.info_inner.total_memory = self.handler.total_memory();
        self.info_inner.used_memory = self.handler.used_memory();
    }

    fn update_disks(&mut self) {
        self.info_inner.hdd_disk_total = 0;
        self.info_inner.hdd_disk_avail = 0;
        self.info_inner.ssd_disk_total = 0;
        self.info_inner.ssd_disk_avail = 0;
        for disk in self.handler.disks() {
            if disk.is_removable() {
                continue;
            }
            match disk.type_() {
                DiskType::HDD => {
                    self.info_inner.hdd_disk_total += disk.total_space();
                    self.info_inner.hdd_disk_avail += disk.available_space();
                }
                DiskType::SSD => {
                    self.info_inner.ssd_disk_total += disk.total_space();
                    self.info_inner.ssd_disk_avail += disk.available_space();
                }
                // In a linux+docker environment, each docker container mount path will be recognized as a separate disk, causing OOD's system info to return an error
                // Here first ensure the correctness of OOD, not to consider the unknown disk as ssd
                // Impact: OOD running under WSL1, the disk size is 0, the mobile stack, the disk size is 0, these turned out to be Unknown
                DiskType::Unknown(_) => {
                    // self.info_inner.ssd_disk_total += disk.total_space();
                    // self.info_inner.ssd_disk_avail += disk.available_space();
                }
            }
        }
    }

    fn update_cpu(&mut self) {
        let info = self.handler.global_cpu_info();
        self.info_inner.cpu_usage = info.cpu_usage();
        self.info_inner.cpu_brand = info.brand().to_owned();
    }

    fn update_mac(&mut self) {
        // Get the first NIC address
        #[cfg(all(not(target_os = "android"), not(target_os = "ios")))]
        let mac_address = match mac_address::get_mac_address() {
            Ok(Some(v)) => Some(v.to_string()),
            Ok(None) => {
                error!("get mac address but not found!");
                None
            }
            Err(e) => {
                error!("get mac address error! {}", e);
                None
            }
        };

        #[cfg(any(target_os = "android", target_os = "ios"))]
        let mac_address = None;

        self.info_inner.mac_address = mac_address;
    }

    fn update_network(&mut self) {
        let networks = self.handler.networks();
        let mut received_bytes = 0;
        let mut transmitted_bytes = 0;
        let mut total_received_bytes = 0;
        let mut total_transmitted_bytes = 0;

        for (interface_name, network) in networks {
            if interface_name
                .find("Hyper-V Virtual Ethernet Adapter")
                .is_some()
            {
                //info!("will ignore as Hyper-V Virtual Ethernet Adapter addr: {}", description);
                continue;
            }

            if interface_name.find("VMware").is_some() {
                //info!("will ignore as VMware addr: {}", description);
                continue;
            }

            if network.mac_address().is_unspecified() {
                warn!(
                    "will ignore unspecified addr network interface: {}",
                    interface_name
                );
                continue;
            }

            // info!("in: {}, total_received_bytes={}, total_transmitted_bytes={}, addr={:?}",
            //    interface_name, network.total_received(), network.total_transmitted(), network.mac_address());

            received_bytes += network.received();
            transmitted_bytes += network.transmitted();

            total_received_bytes += network.total_received();
            total_transmitted_bytes += network.total_transmitted();
        }

        self.info_inner.received_bytes = received_bytes;
        self.info_inner.transmitted_bytes = transmitted_bytes;

        self.info_inner.total_received_bytes = total_received_bytes;
        self.info_inner.total_transmitted_bytes = total_transmitted_bytes;
    }
}

#[derive(Clone)]
pub struct SystemInfoManager(Arc<Mutex<SystemInfoManagerInner>>);

impl SystemInfoManager {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(SystemInfoManagerInner::new())))
    }

    pub async fn get_system_info(&self) -> SystemInfo {
        if !self.0.lock().unwrap().running {
            self.start();
            async_std::task::sleep(Duration::from_secs(2)).await;
        }

        let mut item = self.0.lock().unwrap();
        item.last_access_time = Instant::now();
        item.info_inner.clone()
    }

    pub fn start(&self) {
        let start = {
            let mut item = self.0.lock().unwrap();
            if !item.running {
                item.running = true;
                true
            } else {
                false
            }
        };

        if !start {
            info!("system info already in refreshing!");
            return;
        }

        info!("start refresh system info...");

        let this = self.clone();
        async_std::task::spawn(async move { this.run_refresh().await });
    }

    async fn run_refresh(&self) {
        loop {
            {
                let mut item = self.0.lock().unwrap();
                item.check_idle();
                if !item.running {
                    break;
                }

                item.refresh();
            }

            async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    pub fn stop(&self) {
        let mut item = self.0.lock().unwrap();
        if item.running {
            item.running = false;
            info!("will stop refresh system info!");
        } else {
            info!("refresh system info stopped already!");
        }
    }

    pub fn update_system_info(&self, updater: SystemInfoUpdater) {
        let mut item = self.0.lock().unwrap();
        item.update_system_info(updater);
    }
}

// The global singleton pattern is used here to avoid the cyfs-runtime and cyfs-stack components holding two separate instances
lazy_static::lazy_static! {
    pub static ref SYSTEM_INFO_MANAGER: SystemInfoManager = SystemInfoManager::new();
}
