use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_util::get_cyfs_root_path;

use std::path::{Path, PathBuf};

const CONFIG_FILE_NAME: &str = "debug.toml";

pub struct DebugConfigFile {
    file_name: String,
    config_path: Option<PathBuf>,
}

impl DebugConfigFile {
    pub fn new() -> Self {
        let mut ret = Self {
            file_name: CONFIG_FILE_NAME.to_owned(),
            config_path: None,
        };

        if let Some(file) = ret.search() {
            println!("will use debug config file: {}", file.display());
            ret.config_path = Some(file);
        }

        ret
    }

    fn into(self) -> Option<PathBuf> {
        self.config_path
    }

    fn search(&self) -> Option<PathBuf> {
        match std::env::current_exe() {
            Ok(mut dir) => {
                dir.set_file_name(&self.file_name);

                if dir.is_file() {
                    info!("config found: {}", dir.display());
                    return Some(dir);
                }
            }
            Err(e) => {
                error!("get current_exe error: {}", e);
            }
        };

        match std::env::current_dir() {
            Ok(mut dir) => {
                dir.set_file_name(&self.file_name);

                if dir.is_file() {
                    info!("config found: {}", dir.display());
                    return Some(dir);
                }
            }
            Err(e) => {
                error!("get current_exe error: {}", e);
            }
        };

        // 从/cyfs/etc/目录下面查找
        let root = get_cyfs_root_path();
        let file = root.join(format!("etc/{}", self.file_name));
        if file.is_file() {
            info!("config found: {}", file.display());
            return Some(file);
        }

        warn!("config not found! now will use default config");
        return None;
    }
}

pub struct DebugConfig {
    pub config_file: Option<PathBuf>,
}

impl DebugConfig {
    pub fn new() -> Self {
        let config_file = DebugConfigFile::new();
        Self {
            config_file: config_file.into(),
        }
    }

    pub fn get_config(key: &str) -> Option<&'static toml::Value> {
        use once_cell::sync::OnceCell;
        static S_INSTANCE: OnceCell<BuckyResult<toml::value::Table>> = OnceCell::new();
        let root = S_INSTANCE.get_or_init(|| {
            let config = DebugConfig::new();
            config.load()
        });

        match root {
            Ok(root) => root.get(key),
            Err(_) => None,
        }
    }

    pub fn set_config_file(&mut self, file: &Path) {
        self.config_file = Some(file.to_owned());
    }

    pub fn load(&self) -> BuckyResult<toml::value::Table> {
        if let Some(file) = &self.config_file {
            self.load_root(file)
        } else {
            println!("config file not found!");
            Err(BuckyError::from(BuckyErrorCode::NotFound))
        }
    }

    fn load_root(&self, config_path: &Path) -> BuckyResult<toml::value::Table> {
        let contents = std::fs::read_to_string(config_path).map_err(|e| {
            let msg = format!(
                "load log config failed! file={}, err={}",
                config_path.display(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let cfg_node: toml::Value = toml::from_str(&contents).map_err(|e| {
            let msg = format!(
                "parse log config failed! file={}, content={}, err={}",
                config_path.display(),
                contents,
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        // println!("debug config: {:?}", cfg_node);

        if !cfg_node.is_table() {
            let msg = format!(
                "invalid log config format! file={}, content={}",
                config_path.display(),
                contents,
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        match cfg_node {
            toml::Value::Table(v) => Ok(v),
            _ => unreachable!(),
        }
    }
}
