use crate::config::PATHS;
use crate::service::SERVICE_MANAGER;
use cyfs_base::{BuckyError, BuckyResult, BuckyErrorCode};
use super::service_config::*;

use std::path::PathBuf;

pub struct DeviceConfig {
    config_file: PathBuf,
}

impl DeviceConfig {
    pub fn new() -> DeviceConfig {
        let config_file = PATHS.device_config.to_path_buf();
        DeviceConfig { config_file }
    }

    pub async fn load_and_apply_config(&self) -> BuckyResult<()> {
        let list = self.load_config()?;

        SERVICE_MANAGER.load(list).await
    }

    pub fn load_config(&self) -> BuckyResult<Vec<ServiceConfig>> {
        if !self.config_file.exists() {
            let msg = format!(
                "load device config file not found! file={}",
                self.config_file.display()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let ret = self.load_as_toml();
        if ret.is_err() {
            let e = ret.unwrap_err();
            error!("load config file error, file={}, err={}", self.config_file.display(), e);

            return Err(e);
        }

        self.parse_config(ret.unwrap())
    }

    fn load_as_toml(&self) -> BuckyResult<toml::Value> {
        let s = std::fs::read_to_string(&self.config_file)?;

        let u: toml::Value = toml::from_str(&s).map_err(|e| {
            let msg = format!("invalid device-config.toml format! value={}, {}", s, e);
            error!("{}", e);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(u)
    }

    fn parse_config(&self, cfg_node: toml::Value) -> BuckyResult<Vec<ServiceConfig>> {
        if !cfg_node.is_table() {
            let msg = format!(
                "config root node invalid format! file={}",
                self.config_file.display()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        for (k, v) in cfg_node.as_table().unwrap() {
            match k.as_str() {
                "service" => {
                    if v.is_array() {
                        return ServiceConfig::load_service_list(v.as_array().unwrap());
                    } else {
                        let msg = format!("config invalid service node format");
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                }
                _ => {
                    warn!("unknown device service config node: {}", &k);
                }
            }
        }

        let msg = format!("service node not found in config! {:?}", cfg_node);
        error!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
    }
}
