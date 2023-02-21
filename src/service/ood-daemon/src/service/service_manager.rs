use super::service::Service;
use super::service_info::ServicePackageLocalState;
use crate::config::*;
use crate::daemon::GATEWAY_MONITOR;
use crate::status::*;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_debug::Mutex;
use cyfs_lib::ZoneRole;

use async_std::sync::Mutex as AsyncMutex;
use lazy_static::lazy_static;
use std::fmt;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, str::FromStr};

#[derive(Clone)]
pub struct ServiceItem {
    pub config: ServiceConfig,
    pub service: Option<Arc<Service>>,
}

impl ServiceItem {
    pub fn target_state(&self) -> ServiceState {
        match self.config.enable {
            true => match GATEWAY_MONITOR.zone_role() {
                ZoneRole::ActiveOOD => self.config.target_state,
                _ => match self.config.name.as_str() {
                    GATEWAY_SERVICE => ServiceState::Run,
                    _ => ServiceState::Stop,
                },
            },
            false => ServiceState::Stop,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ServiceMode {
    Installer = 0,
    Daemon = 1,

    // 虚拟ood模式，不需要同步ood-daemon服务
    VOOD = 2,

    // cyfs-runtime模式
    Runtime = 3,
}

impl fmt::Display for ServiceMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mode = match &self {
            ServiceMode::Installer => "installer",
            ServiceMode::Daemon => "daemon",
            ServiceMode::VOOD => "vood",
            ServiceMode::Runtime => "runtime",
        };

        write!(f, "{}", mode)
    }
}

impl FromStr for ServiceMode {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<ServiceMode> {
        let ret = match s {
            "installer" => ServiceMode::Installer,
            "daemon" => ServiceMode::Daemon,
            "vood" => ServiceMode::VOOD,
            "runtime" => ServiceMode::Runtime,
            v @ _ => {
                let msg = format!("unknown service mode: {}", v);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        Ok(ret)
    }
}

pub struct ServiceManager {
    mode: Arc<Mutex<ServiceMode>>,

    // 是否开启旧安装gc
    enable_gc: Arc<Mutex<bool>>,

    service_list: Arc<Mutex<HashMap<String, ServiceItem>>>,
    service_root: PathBuf,

    sync_lock: AsyncMutex<i32>,
}

impl ServiceManager {
    fn new() -> Self {
        Self {
            mode: Arc::new(Mutex::new(ServiceMode::Daemon)),
            enable_gc: Arc::new(Mutex::new(true)),
            service_list: Arc::new(Mutex::new(HashMap::new())),
            service_root: PATHS.service_root.clone(),
            sync_lock: AsyncMutex::new(0),
        }
    }

    pub fn get_mode(&self) -> ServiceMode {
        *self.mode.lock().unwrap()
    }

    pub fn change_mode(&self, new_mode: ServiceMode) -> ServiceMode {
        let mut mode = self.mode.lock().unwrap();
        if *mode != new_mode {
            let old_mode = *mode;
            *mode = new_mode;

            info!("service mode changed: {} => {}", old_mode, mode);
            old_mode
        } else {
            *mode
        }
    }

    pub fn is_enable_gc(&self) -> bool {
        *self.enable_gc.lock().unwrap()
    }

    pub fn enable_gc(&self, enable: bool) -> bool {
        let mut cur = self.enable_gc.lock().unwrap();
        if *cur != enable {
            let old = *cur;
            *cur = enable;

            info!("service enable gc changed: {} => {}", old, enable);
            old
        } else {
            *cur
        }
    }

    pub async fn load(&self, list: Vec<ServiceConfig>) -> BuckyResult<()> {
        // 根据当前servicemode，来确定操作类型
        match self.get_mode() {
            ServiceMode::Daemon => {
                // daemon模式下，ood_daemon当作普通服务来处理，用以安装包的更新，
                // service内部会在sync_state时候过滤ood_daemon
                self.sync_service_list(list).await?;
            }
            ServiceMode::VOOD => {
                // vood模式下，不需要处理ood-daemon的更新和启动
                // 04/16 vood暂时屏蔽chunk-manager和file-manager服务
                let list: Vec<ServiceConfig> = list
                    .into_iter()
                    .filter_map(|v| {
                        if v.name == OOD_DAEMON_SERVICE
                            || v.name == "chunk-manager"
                            || v.name == "file-manager"
                        {
                            None
                        } else {
                            Some(v)
                        }
                    })
                    .collect();
                self.sync_service_list(list).await?;
            }
            ServiceMode::Installer => {
                let mut ood_daemon_service: Option<ServiceConfig> = None;
                for item in list {
                    if item.name == OOD_DAEMON_SERVICE {
                        ood_daemon_service = Some(item);
                        break;
                    }
                }

                if ood_daemon_service.is_some() {
                    // installer模式下，只需要处理ood_daemon服务
                    self.sync_service_list(vec![ood_daemon_service.unwrap()])
                        .await?;
                } else {
                    let msg = format!("ood-daemon not found in device_config!");
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }
            }
            ServiceMode::Runtime => {
                unreachable!();
            }
        };

        Ok(())
    }

    pub fn get_service_info(&self, name: &str) -> Option<ServiceItem> {
        self.service_list
            .lock()
            .unwrap()
            .get(name)
            .map(|item| item.to_owned())
    }

    // 同步服务列表
    async fn sync_service_list(&self, service_config_list: Vec<ServiceConfig>) -> BuckyResult<()> {
        let _lock = self.sync_lock.lock().await;

        // 查找被移除的service
        let mut remove_list = Vec::new();
        for (_, current_service_info) in self.service_list.lock().unwrap().iter() {
            if !service_config_list
                .iter()
                .any(|item| item.name == current_service_info.config.name)
            {
                remove_list.push(current_service_info.config.name.to_owned());
            }
        }

        for name in remove_list {
            self.on_remove_service(&name);
        }

        // 查找新增的和发生改变的service
        for service_config in service_config_list {
            if self.get_service_info(&service_config.name).is_none() {
                self.on_new_service(service_config).await?;
            } else {
                self.sync_service(&service_config).await?;
            }
        }

        info!("sync service list success!");

        Ok(())
    }

    // 需要移除此服务
    fn on_remove_service(&self, name: &str) {
        info!("will remove service: {}", name);

        let service_info = self.service_list.lock().unwrap().remove(name);
        assert!(service_info.is_some());
        let service_info = service_info.unwrap();

        let service = service_info.service.unwrap();
        service.sync_state(ServiceState::Stop);
        service.remove();
    }

    // 发现了新的服务，需要加载并同步为目标状态
    async fn on_new_service(&self, mut service_config: ServiceConfig) -> BuckyResult<()> {
        info!(
            "will add new service: {}, {}, {}",
            service_config.name, service_config.fid, service_config.version,
        );

        let ret = self.create_service(&mut service_config).await;
        if let Err(e) = ret {
            error!(
                "create service failed! name={}, e={}",
                service_config.name, e
            );
            return Err(e);
        }

        let service_item = ServiceItem {
            config: service_config,
            service: Some(Arc::new(ret.unwrap())),
        };

        // 同步service状态
        Self::sync_service_target_state(&service_item);

        // 添加进列表
        let ret = self
            .service_list
            .lock()
            .unwrap()
            .insert(service_item.config.name.clone(), service_item);
        assert!(ret.is_none());

        Ok(())
    }

    async fn create_service(&self, service_config: &ServiceConfig) -> BuckyResult<Service> {
        // 根据service_info，创建service
        let mut service = Service::new(&service_config);

        // 绑定root
        let service_path = self.service_root.join(&service_config.name);
        if !service_path.is_dir() {
            if let Err(e) = std::fs::create_dir_all(service_path.as_path()) {
                let msg = format!(
                    "create service dir error! dir={}, err={}",
                    service_path.display(),
                    e
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        service.bind(service_path.as_path());

        // 尝试初始化
        if let Err(e) = service.init().await {
            return Err(e);
        }

        // 初始化成功，同步到目标状态
        service.sync_state(service_config.target_state);

        // 尝试清空旧的安装包
        if self.is_enable_gc() {
            let fid = service.fid().to_owned();
            async_std::task::spawn(async move {
                // 避免删除正在运行的服务目录，导致调用stop命令出错，这里延迟一会再删除
                async_std::task::sleep(std::time::Duration::from_secs(60 * 1)).await;

                use super::local_package_manager::LocalPackageManager;
                let lmp = LocalPackageManager::new(service_path);
                let _ = lmp.gc(vec![fid]).await;
            });
        }

        Ok(service)
    }

    fn update_service_info(&self, service_config: &ServiceConfig) {
        let mut coll = self.service_list.lock().unwrap();
        let current_service_info = coll.get_mut(&service_config.name).unwrap();
        current_service_info.config.update(service_config);
    }

    async fn sync_service(&self, service_config: &ServiceConfig) -> BuckyResult<()> {
        let current_service_info = self.get_service_info(&service_config.name).unwrap();
        assert_eq!(current_service_info.config.name, service_config.name);

        // 首先检查文件是否发生改变
        if current_service_info.config.fid != service_config.fid {
            info!(
                "service package changed! name={}, old={}, new={}",
                current_service_info.config.name,
                current_service_info.config.fid,
                service_config.fid
            );

            self.on_service_package_changed(service_config).await?;
            // 同步成功后，再更新存储的service_info
            self.update_service_info(service_config);
            return Ok(());
        }

        // enable状态发生改变了
        if current_service_info.config.enable != service_config.enable {
            info!(
                "service enable changed! name={}, old={}, new={}",
                current_service_info.config.name,
                current_service_info.config.enable,
                service_config.enable
            );

            self.update_service_info(service_config);

            self.on_service_enable_changed(&service_config.name);

            return Ok(());
        }

        // 目标运行状态发生改变了
        if current_service_info.config.target_state != service_config.target_state {
            info!(
                "service target_state changed! name={}, old={}, new={}",
                current_service_info.config.name,
                current_service_info.config.target_state,
                service_config.target_state
            );

            self.update_service_info(service_config);

            self.on_service_target_state_changed(&service_config.name);

            return Ok(());
        }

        self.update_service_info(service_config);

        Ok(())
    }

    async fn on_service_package_changed(&self, service_config: &ServiceConfig) -> BuckyResult<()> {
        let ret = self.create_service(service_config).await;
        if let Err(e) = ret {
            error!(
                "create service failed! service={}, err={}",
                service_config.name, e
            );
            return Err(e);
        }

        // 检查service的包是否同步成功了，如果同步失败那么不替换本地服务
        let new_service = Arc::new(ret.unwrap());
        if !new_service.check_package() {
            let msg = format!(
                "service package check failed! service={}",
                service_config.name,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let old_service;
        let service_item;
        {
            let mut coll = self.service_list.lock().unwrap();
            let current_service_info = coll.get_mut(&service_config.name).unwrap();
            old_service = current_service_info.service.take();

            current_service_info.service = Some(new_service);
            service_item = current_service_info.clone();
        }

        // 首先停止老的服务
        if let Some(old_service) = old_service {
            old_service.sync_state(ServiceState::Stop);
        }

        // 尝试启动新的服务
        Self::sync_service_target_state(&service_item);

        Ok(())
    }

    fn on_service_enable_changed(&self, name: &str) {
        let service_item = self.get_service_info(name).unwrap();
        Self::sync_service_target_state(&service_item);
    }

    fn on_service_target_state_changed(&self, name: &str) {
        let service_item = self.get_service_info(name).unwrap();
        Self::sync_service_target_state(&service_item);
    }

    pub async fn sync_all_service_state(&self) {
        let lock = self.sync_lock.try_lock();
        if lock.is_none() {
            info!("sync all service state but already in sync service packages!");
            return;
        }

        info!("will sync all service state");
        let service_list = self.service_list.lock().unwrap().clone();
        for (name, service_item) in service_list {
            if service_item.service.is_none() {
                error!("service not init! name={}", name);
                continue;
            }

            Self::sync_service_target_state(&service_item);
        }
    }

    pub async fn sync_service_packages(&self) {
        let _lock = self.sync_lock.lock().await;

        debug!("will sync all service packages");

        let service_list = self.service_list.lock().unwrap().clone();
        for (name, service_info) in service_list {
            if service_info.service.is_none() {
                error!("service not init yet! name={}", name);
                continue;
            }

            if service_info.target_state() == ServiceState::Run {
                if let Err(e) = Self::sync_service_package(&service_info).await {
                    error!(
                        "sync service package failed! service={}, {}",
                        service_info.config.name, e
                    );
                }
            }
        }
    }

    async fn sync_service_package(service_info: &ServiceItem) -> BuckyResult<()> {
        let service = service_info.service.as_ref().unwrap();
        match service.sync_package().await {
            Ok(changed) => {
                if changed {
                    // 更新成功包，需要同步状态
                    Self::sync_service_target_state(&service_info);
                }

                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn sync_service_target_state(service_item: &ServiceItem) {
        let target_state = service_item.target_state();

        let service = service_item.service.as_ref().unwrap();
        service.sync_state(target_state);
    }

    pub fn collect_status(&self) -> Vec<OODServiceStatusItem> {
        let mut list = vec![];
        let services = self.service_list.lock().unwrap();
        for (_name, item) in services.iter() {
            let package_state = match &item.service {
                Some(service) => service.package_local_status(),
                None => ServicePackageLocalState::Init,
            };

            let process_state = match &item.service {
                Some(service) => service.state(),
                None => ServiceState::Stop,
            };

            let item = OODServiceStatusItem {
                id: item.config.id.clone(),
                name: item.config.name.clone(),
                version: item.config.version.clone(),
                enable: item.config.enable,
                target_state: item.config.target_state,
                package_state,
                process_state,
            };

            list.push(item);
        }

        list
    }
}

lazy_static! {
    pub static ref SERVICE_MANAGER: ServiceManager = {
        return ServiceManager::new();
    };
}
