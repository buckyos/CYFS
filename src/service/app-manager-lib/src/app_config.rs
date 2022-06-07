use crate::def::*;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, APP_MANAGER_NAME};

use log::*;
use serde::{Deserialize};
use std::str::FromStr;


/*
[config]
host_mode = "dev"
*/

#[derive(Deserialize)]
struct ConfigNode {
    host_mode: Option<String>,
}

#[derive(Deserialize)]
struct AppManagerConfigNode {
    config: ConfigNode,
}

pub struct AppManagerConfig {
    host_mode: AppManagerHostMode,
}

impl AppManagerConfig {
    pub fn new() -> Self {
        let mut ret = Self {
            host_mode: AppManagerHostMode::default(),
        };

        // FIXME what to do if load error? now will ignore and use default values
        let _ = ret.load();

        if *ret.host_mode() == AppManagerHostMode::Dev {
            warn!(">>>>>>>>app-manager running in dev mode!>>>>>>>>")
        }
        ret
    }

    //windows默认不docker，其他系统根据配置来，默认使用
    pub fn host_mode(&self) -> &AppManagerHostMode {
        &self.host_mode
    }

    fn load(&mut self) -> BuckyResult<()> {
        let node = Self::load_config_file()?;
        if node.is_none() {
            return Ok(());
        }

        let node = node.unwrap();
        if let Some(v) = node.config.host_mode {
           if let Ok(mode) = AppManagerHostMode::from_str(&v) {
               info!("will use host_mode in config file! mode={}", v);
               self.host_mode = mode;
           }
        }

        Ok(())
    }

    fn load_config_file() -> BuckyResult<Option<AppManagerConfigNode>> {
        let config_file = cyfs_util::get_cyfs_root_path().join("etc")
        .join(APP_MANAGER_NAME)
        .join(CONFIG_FILE_NAME);

        if !config_file.is_file() {
            return Ok(None);
        }

        let contents = std::fs::read_to_string(&config_file).map_err(|e| {
            let msg = format!(
                "load app-manager config failed! file={}, err={}",
                config_file.display(),
                e
            );
            info!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let config: AppManagerConfigNode = toml::from_str(&contents).map_err(|e| {
            let msg = format!(
                "parse app-manager config failed! file={}, content={}, err={}",
                config_file.display(),
                contents,
                e
            );
            info!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(Some(config))
    }
}