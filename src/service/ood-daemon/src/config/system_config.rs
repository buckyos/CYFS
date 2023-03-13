use super::path::PATHS;
use super::version::{ServiceListVersion, ServiceVersion};
use crate::repo::REPO_MANAGER;
use cyfs_base::*;
use cyfs_util::TomlHelper;
use super::monitor::SystemConfigMonitor;

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Eq, PartialEq)]
pub struct SystemConfig {
    // device_config动态更新依赖的desc文件，默认从cyfs_repo，可以指定device标识当前设备
    // ServiceList对象会使用该id+version，来计算当前依赖的ServiceList对象，并从链上拉取
    pub config_desc: String,

    // service list version
    pub service_list_version: ServiceListVersion,

    // service version
    pub service_version: ServiceVersion,

    // enable preview
    pub preview: bool,

    // 当前平台对应的target
    pub target: String,
}

impl SystemConfig {
    fn new() -> Self {
        Self {
            config_desc: String::from("cyfs_repo"),

            service_list_version: ServiceListVersion::default(),

            service_version: ServiceVersion::default(),
            preview: false,

            target: String::from(""),
        }
    }

    // return true if the same
    pub fn compare(&self, other: &SystemConfig) -> bool {
        *self == *other
    }

    pub async fn load_config(&mut self) -> BuckyResult<()> {
        let config_file = PATHS.system_config.clone();

        if !config_file.exists() {
            let msg = format!(
                "load system config file not found! file={}",
                config_file.display()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        debug!("will load system-config file: {}", config_file.display());

        let node = Self::load_as_toml(&config_file)?;

        self.parse_config(node).await
    }

    pub fn load_as_toml(file_path: &Path) -> BuckyResult<toml::Value> {
        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| {
                let msg = format!(
                    "load system config to string error! file={}, {}",
                    file_path.display(),
                    e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let node = toml::from_str(&content).map_err(|e| {
            let msg = format!(
                "load system config invalid format! content={}, file={}, {}",
                content,
                file_path.display(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(node)
    }

    async fn parse_config(&mut self, cfg_node: toml::Value) -> BuckyResult<()> {
        if !cfg_node.is_table() {
            let msg = format!(
                "config root node invalid format! file={}",
                PATHS.system_config.display()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        for (k, v) in cfg_node.as_table().unwrap() {
            match k.as_str() {
                "device" => {
                    if v.is_table() {
                        self.load_device_info(v.as_table().unwrap())?;
                    } else {
                        let msg = format!("config invalid device node format");
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                }
                "repository" => {
                    if v.is_array() {
                        REPO_MANAGER.load(v.as_array().unwrap()).await?;
                    } else {
                        let msg = format!("config invalid repository node format");
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                }
                _ => {
                    warn!("unknown system config node: {}", &k);
                }
            }
        }

        Ok(())
    }

    pub fn load_device_info(&mut self, device_node: &toml::value::Table) -> BuckyResult<()> {
        for (k, v) in device_node.iter() {
            match k.as_str() {
                "config_desc" => {
                    self.config_desc = TomlHelper::decode_from_string(v)?;
                }
                "service_list_version" => {
                    let v: String = TomlHelper::decode_from_string(v)?;

                    self.service_list_version = ServiceListVersion::from_str(v.trim())?;
                }
                "service_version" => {
                    let v: String = TomlHelper::decode_from_string(v)?;

                    self.service_version = ServiceVersion::from_str(v.trim())?;
                }
                "preview" => {
                    self.preview = TomlHelper::decode_from_boolean(v)?;
                }
                "target" => {
                    self.target = TomlHelper::decode_from_string(v)?;
                }
                _ => {}
            }
        }

        if self.target.is_empty() {
            let msg = format!("target not specified!");
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        Ok(())
    }
}

use std::sync::Mutex;
static SYSTEM_CONFIG: Mutex<Option<Arc<SystemConfig>>> = Mutex::new(None);

// 只在进程初始化时候调用一次
pub async fn init_system_config() -> BuckyResult<()> {
    let mut system_config = SystemConfig::new();
    system_config.load_config().await?;

    info!("init system config: {:?}", system_config);
    *SYSTEM_CONFIG.lock().unwrap() = Some(Arc::new(system_config));

    Ok(())
}

pub async fn reload_system_config() -> BuckyResult<bool> {
    let mut system_config = SystemConfig::new();
    system_config.load_config().await?;

    debug!("reload system config success! {:?}", system_config);
    let mut changed = false;
    {
        let mut current = SYSTEM_CONFIG.lock().unwrap();
        if current.as_deref() != Some(&system_config) {
            info!(
                "reload system config and changed! {:?} -> {:?}",
                current.as_deref(), system_config
            );
            *current = Some(Arc::new(system_config));
            changed = true;
        }
    }

    Ok(changed)
}

pub fn get_system_config() -> Arc<SystemConfig> {
    SYSTEM_CONFIG.lock().unwrap().clone().unwrap()
}