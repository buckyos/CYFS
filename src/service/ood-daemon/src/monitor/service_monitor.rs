use crate::config::{ServiceConfig, ServiceState};
use crate::service::Service;
use cyfs_base::{BuckyError, BuckyResult};

use async_std::task;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) struct ServiceMonitor {
    name: String,
    service_root: PathBuf,

    // 当前的版本，使用fid来唯一区分
    version: String,

    service: Option<Service>,
}

impl ServiceMonitor {
    pub fn new(service_name: &str) -> Self {
        let file_path = ::cyfs_util::get_cyfs_root_path();
        let service_root = file_path.join("services").join(service_name);

        Self {
            name: service_name.to_owned(),
            service_root,
            version: "".to_owned(),
            service: None,
        }
    }

    pub fn start_monitor(service_name: &str) -> BuckyResult<()> {
        // Self::launch_monitor();

        Self::run_monitor_checker(service_name)
    }

    fn try_get_cmd_from_config() -> Option<String> {
        match crate::SERVICE_MANAGER
            .get_service_info(::cyfs_base::OOD_DAEMON_NAME)
            .map(|v| v.service)
        {
            Some(Some(service)) => service
                .get_script("start")
                .map(|v| format!("{} --as-monitor", v)),
            _ => {
                None
            }
        }
    }

    fn try_get_cmd_from_exe() -> BuckyResult<String> {
        let ret = std::env::current_exe();
        if ret.is_err() {
            let msg = format!("get current exe error: {}", ret.unwrap_err());
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        let exe_file = ret.unwrap();
        let cmd_line = format!(r#""{}" --as-monitor"#, exe_file.to_str().unwrap());

        Ok(cmd_line)
    }

    pub fn launch_monitor() -> Result<(), BuckyError> {
        let cmd_line = match Self::try_get_cmd_from_config() {
            Some(cmd) => cmd,
            None => Self::try_get_cmd_from_exe()?,
        };

        info!("will launch daemon monitor: {}", cmd_line);

        cyfs_util::process::launch_as_daemon(&cmd_line)
    }

    fn run_monitor_checker(service_name: &str) -> BuckyResult<()> {
        let name = Self::monitor_name(service_name);

        let mutex = cyfs_util::process::ProcessMutex::new(&name);
        if mutex.acquire().is_some() {
            warn!("monitor process not exists! will restart...");
            Self::launch_monitor()?;
        }

        task::spawn(async move {
            loop {
                task::sleep(Duration::from_secs(5)).await;

                if mutex.acquire().is_some() {
                    warn!("monitor process not exists! will restart...");
                    let _ = Self::launch_monitor();
                }
            }
        });

        Ok(())
    }

    fn monitor_name(service_name: &str) -> String {
        format!("{}-monitor", service_name)
    }

    pub fn run_as_monitor(service_name: &str) {
        let monitor_service_name = Self::monitor_name(service_name);
        if !::cyfs_util::process::try_enter_proc(&monitor_service_name) {
            info!("monitor already running!");
            std::process::exit(1);
        }

        cyfs_debug::CyfsLoggerBuilder::new_service(&monitor_service_name)
            .level("info")
            .console("info")
            .enable_bdt(Some("debug"), Some("debug"))
            .build()
            .unwrap()
            .start();

        cyfs_debug::PanicBuilder::new("cyfs-service", &monitor_service_name)
            .build()
            .start();

        let monitor = ServiceMonitor::new(service_name);
        monitor.run();
    }

    // 尝试终止monitor进程
    pub fn stop_monitor_process(service_name: &str) {
        let monitor_service_name = Self::monitor_name(service_name);
        if cyfs_util::process::check_process_mutex(&monitor_service_name) {
            info!("will stop monitor process: {}", service_name);
            cyfs_util::process::try_stop_process(&monitor_service_name);
        } else {
            info!("monitor process not running: {}", service_name);
        }
    }

    fn run(mut self) {
        loop {
            std::thread::sleep(Duration::from_secs(60));

            trace!("now will check once");

            if let Err(e) = self.check() {
                error!("monitor check error: {} {}", self.version, e);
            }
        }
    }

    fn check(&mut self) -> Result<(), BuckyError> {
        if self.service.is_some() {
            self.service.as_mut().unwrap().update_state();
            if self.service.as_ref().unwrap().state() == ServiceState::RUN {
                return Ok(());
            }

            info!("service is not running: {}", self.version);
            self.service = None;
        }

        // 重新读取version文件，获取fid
        let version = self.load_version()?;
        self.version = version;

        let service = self.new_service(&self.version)?;

        service.direct_sync_state(ServiceState::RUN);

        self.service = Some(service);

        Ok(())
    }

    fn load_version(&self) -> Result<String, BuckyError> {
        let version_file = self.service_root.join("version");
        if !version_file.is_file() {
            let msg = format!("version file not exists: {}", version_file.display());
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        std::fs::read_to_string(&version_file).map_err(|e| {
            error!("read version file error: {} {}", version_file.display(), e);

            BuckyError::from(e)
        })
    }

    fn new_service(&self, version: &str) -> Result<Service, BuckyError> {
        let current_root = self.service_root.join(version);
        if !current_root.is_dir() {
            let msg = format!("current root dir not exists: {}", current_root.display());
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        info!(
            "load daemon service: version={}, dir={}",
            version,
            current_root.display()
        );

        let service_config = ServiceConfig {
            id: "".to_owned(),
            name: self.name.clone(),
            fid: version.to_owned(),
            version: "0.0.1".to_owned(),
            enable: true,
            target_state: ServiceState::RUN,
        };
        let mut service = Service::new(&service_config);
        service.mark_ood_daemon(false);
        service.bind(&self.service_root);

        // 加载包配置
        service.load_package()?;

        Ok(service)
    }
}
