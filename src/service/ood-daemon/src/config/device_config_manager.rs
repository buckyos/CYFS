use super::get_system_config;
use super::service_config::ServiceConfig;
use super::DeviceConfig;
use super::PATHS;
use crate::config_repo::*;
use cyfs_base::*;
use cyfs_debug::Mutex;

use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use std::time::Duration;

pub struct DeviceConfigManager {
    repo: OnceCell<DeviceConfigRepoRef>,

    device_config_hash: Mutex<Option<HashValue>>,
    device_config: DeviceConfig,
}

impl DeviceConfigManager {
    pub fn new() -> Self {
        Self {
            repo: OnceCell::new(),
            device_config_hash: Mutex::new(None),
            device_config: DeviceConfig::new(),
        }
    }

    pub fn init(&self) -> BuckyResult<()> {
        // 从system_config获取device_config依赖的desc
        let desc = &get_system_config().config_desc;

        info!("will init device_config repo: {}", desc);

        let repo = if desc == "local" {
            let repo = DeviceConfigLocalRepo::new();
            Box::new(repo) as Box<dyn DeviceConfigRepo>
        } else if desc == "cyfs_repo" || desc == "device" {
            let mut repo = DeviceConfigMetaRepo::new();
            if let Err(e) = repo.init(&desc, &get_system_config().service_list_version) {
                return Err(e);
            }

            Box::new(repo) as Box<dyn DeviceConfigRepo>
        } else if desc.starts_with("http") {
            let repo = DeviceConfigHttpRepo::new(&desc)?;

            Box::new(repo) as Box<dyn DeviceConfigRepo>
        } else {
            let msg = format!("invalid device-config dep desc: {}", desc);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
        };

        if let Err(_) = self.repo.set(Arc::new(repo)) {
            unreachable!();
        }

        // 计算当前device_config的hash
        let hash = Self::calc_config_hash();
        *self.device_config_hash.lock().unwrap() = hash;

        Ok(())
    }

    pub async fn load_and_apply_config(&self) -> BuckyResult<()> {
        self.device_config
            .load_and_apply_config()
            .await
            .map_err(|e| {
                error!("load device config failed! err={}", e);
                e
            })
    }

    pub fn load_config(&self) -> BuckyResult<Vec<ServiceConfig>> {
        self.device_config.load_config().map_err(|e| {
            error!("load device config failed! err={}", e);
            e
        })
    }

    // 计算本地文件的hash
    fn calc_config_hash() -> Option<HashValue> {
        let config_file = &PATHS.device_config;

        if !config_file.exists() {
            let msg = format!(
                "load device config file not found! file={}",
                config_file.display()
            );
            error!("{}", msg);
            return None;
        }

        match cyfs_base::hash_file_sync(&config_file) {
            Ok((hash_value, _len)) => {
                info!(
                    "current config file hash is path={}, hash={}",
                    config_file.display(),
                    hash_value
                );

                Some(hash_value)
            }
            Err(e) => {
                error!(
                    "read config file error! file={}, err={}",
                    config_file.display(),
                    e
                );
                None
            }
        }
    }

    pub async fn fetch_config(&self) -> Result<bool, BuckyError> {
        let repo = self.repo.get().unwrap().clone();

        // 从mete-chain拉取对应desc
        let ret = async_std::future::timeout(Duration::from_secs(60), repo.fetch()).await;

        if ret.is_err() {
            let msg = format!("fetch device config timeout! repo={}", repo.get_type());
            error!("{}", msg);

            return Err(BuckyError::from((BuckyErrorCode::Timeout, msg)));
        }

        let device_config_str = match ret.unwrap() {
            Ok(v) => v,
            Err(e) => {
                let msg = format!("load desc from repo failed! err={}", e);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        debug!(
            "load device_config from {} repo: {}",
            repo.get_type(),
            device_config_str
        );

        // 计算hash并比较是否相同
        let hash_value = cyfs_base::hash_data(device_config_str.as_bytes());
        let new_hash = Some(hash_value.clone());
        if self.device_config_hash.lock().unwrap().eq(&new_hash) {
            info!("device_config not changed! hash={}", hash_value);
            return Ok(false);
        }

        info!(
            "device config changed from {:?} to {}",
            self.device_config_hash.lock().unwrap(),
            hash_value
        );

        info!("device config: {}", device_config_str);

        // 保存到本地配置文件
        Self::save_config_file(device_config_str.as_bytes()).await?;

        // 保存成功后，更新hash
        *self.device_config_hash.lock().unwrap() = new_hash;

        Ok(true)
    }

    // 保存配置文件到本地
    async fn save_config_file(buf: &[u8]) -> BuckyResult<()> {
        let config_file = &PATHS.device_config;

        async_std::fs::write(config_file, buf).await.map_err(|e| {
            error!(
                "write to device config file failed! file={}, err={}",
                config_file.display(),
                e
            );
            BuckyError::from(e)
        })
    }
}

lazy_static! {
    pub static ref DEVICE_CONFIG_MANAGER: DeviceConfigManager = DeviceConfigManager::new();
}
