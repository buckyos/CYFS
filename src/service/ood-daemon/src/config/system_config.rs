use super::path::PATHS;
use super::version::ServiceListVersion;
use crate::repo::REPO_MANAGER;
use cyfs_base::*;
use cyfs_util::TomlHelper;

use std::path::Path;
use std::sync::Arc;
use std::str::FromStr;

#[derive(Debug)]
pub struct SystemConfig {
    // device_config动态更新依赖的desc文件，默认从cyfs_repo，可以指定device标识当前设备
    // ServiceList对象会使用该id+version，来计算当前依赖的ServiceList对象，并从链上拉取
    pub config_desc: String,

    // 版本
    pub version: ServiceListVersion,

    // 当前平台对应的target
    pub target: String,
}

impl SystemConfig {
    pub fn new() -> Self {
        Self {
            config_desc: String::from("cyfs_repo"),
            version: ServiceListVersion::default(),
            target: String::from(""),
        }
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

        info!("will load system-config file: {}", config_file.display());

        let node = self.load_as_json(&config_file).await?;

        self.parse_config(node).await
    }

    async fn load_as_json(&self, file_path: &Path) -> BuckyResult<toml::Value> {
        let content = async_std::fs::read_to_string(&file_path).await.map_err(|e| {
            let msg = format!("load system config to string error! file={}, {}", file_path.display(), e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let node = toml::from_str(&content).map_err(|e| {
            let msg = format!("load system config invalid format! content={}, file={}, {}", content, file_path.display(), e);
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
                        REPO_MANAGER
                            .load(v.as_array().unwrap())
                            .await?;
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
                "version" => {
                    let v: String =  TomlHelper::decode_from_string(v)?;
                    self.version = ServiceListVersion::from_str(&v)?;
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

        info!(
            "system-config: config_desc={}, target={}",
            self.config_desc, self.target
        );

        Ok(())
    }
}

use once_cell::sync::OnceCell;
static SYSTEM_CONFIG: OnceCell<Arc<SystemConfig>> = OnceCell::new();

// 只在进程初始化时候调用一次
pub async fn init_system_config() -> BuckyResult<()> {

    let mut system_config = SystemConfig::new();
    system_config.load_config().await?;

    SYSTEM_CONFIG.set(Arc::new(system_config)).unwrap();
    
    Ok(())
}

pub fn get_system_config() -> Arc<SystemConfig> {
    SYSTEM_CONFIG.get().unwrap().clone()
}