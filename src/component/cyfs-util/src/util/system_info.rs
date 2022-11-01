use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{CpuExt, DiskExt, DiskType, NetworkExt, RefreshKind, System, SystemExt};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub name: String,

    pub cpu_usage: f32,

    // memory size in bytes
    pub total_memory: u64,
    pub used_memory: u64,

    // 每个刷新周期之间的传输的bytes
    pub received_bytes: u64,
    pub transmitted_bytes: u64,

    // SSD硬盘容量和可用容量，包括Unknown, in bytes
    pub ssd_disk_total: u64,
    pub ssd_disk_avail: u64,

    // HDD硬盘容量和可用容量, in bytes
    pub hdd_disk_total: u64,
    pub hdd_disk_avail: u64,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            name: "".to_owned(),
            cpu_usage: 0.0,
            total_memory: 0,
            used_memory: 0,
            received_bytes: 0,
            transmitted_bytes: 0,
            ssd_disk_total: 0,
            ssd_disk_avail: 0,
            hdd_disk_total: 0,
            hdd_disk_avail: 0,
        }
    }
}

struct SystemInfoManagerInner {
    running: bool,
    last_access_time: Instant,
    max_idle_time: Duration,

    info_inner: SystemInfo,
    handler: System,
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
        }
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
        self.update_memory();
        self.update_cpu();
        self.update_network();
        self.update_disks();
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
                // 在linux+docker环境下，每个docker container挂载的路径会被认作一个单独的磁盘，导致OODsystem info返回错误
                // 这里先保证OOD的正确性，不把Unknown的磁盘认为是ssd
                // 影响：在WSL1下运行的OOD，磁盘大小是0，移动端的协议栈，磁盘大小是0，这些原来都是Unknown的
                DiskType::Unknown(_) => {
                    // self.info_inner.ssd_disk_total += disk.total_space();
                    // self.info_inner.ssd_disk_avail += disk.available_space();
                }
            }
        }
    }

    fn update_cpu(&mut self) {
        self.info_inner.cpu_usage = self.handler.global_cpu_info().cpu_usage();
    }

    fn update_network(&mut self) {
        let networks = self.handler.networks();
        let mut received_total = 0;
        let mut transmitted_total = 0;
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

            //trace!("in: {}, {} bytes", interface_name, network.get_received());
            received_total += network.received();
            transmitted_total += network.transmitted();
        }

        self.info_inner.received_bytes = received_total;
        self.info_inner.transmitted_bytes = transmitted_total;
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
}

// 这里使用全局单例模式，避免runtime和non-stack不同的接口分别持有两个实例
lazy_static::lazy_static! {
    pub static ref SYSTEM_INFO_MANAGER: SystemInfoManager = SystemInfoManager::new();
}
