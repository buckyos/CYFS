use std::collections::HashMap;
use std::path::Path;
use crate::def::*;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, APP_MANAGER_NAME};
use cyfs_core::DecAppId;

use log::*;
use serde::{Deserialize, Serialize};

/*
[config]
sandbox = "default"    // default\no\docker
repo_mode = "local:/cyfs/app_repo"     // named_data/local:{local_path}

[app]
include = []
exclude = []
source = "all"              // all\system\user

[app.sandbox]
id1 = "no"
id2 = "docker"
*/

#[derive(Clone, Serialize, Deserialize)]
pub struct AppManagerConfig {
    #[serde(default)]
    pub config: ManagerConfig,

    #[serde(default)]
    pub app: AppConfig,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ManagerConfig {
    #[serde(default)]
    pub sandbox: SandBoxMode,
    #[serde(default)]
    pub repo_mode: RepoMode
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            sandbox: SandBoxMode::default(),
            repo_mode: RepoMode::default()
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub include: Vec<DecAppId>,
    #[serde(default)]
    pub exclude: Vec<DecAppId>,
    #[serde(default)]
    pub source: AppSource,

    #[serde(default)]
    pub sandbox: HashMap<DecAppId, SandBoxMode>,
}

impl AppConfig {
    pub fn can_install_system(&self) -> bool {
        self.source != AppSource::User
    }

    pub fn can_install_user(&self) -> bool {
        self.source != AppSource::System
    }

    pub fn use_docker(&self) -> bool {
        for (_, mode) in &self.sandbox {
            if *mode == SandBoxMode::Docker {
                return true;
            }
        }
        false
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            include: vec![],
            exclude: vec![],
            source: AppSource::All,
            sandbox: HashMap::new(),
        }
    }
}

impl Default for AppManagerConfig {
    fn default() -> Self {
        Self {
            config: ManagerConfig::default(),
            app: AppConfig::default(),
        }
    }
}

impl AppManagerConfig {
    pub fn load() -> Self {
        let config_file = cyfs_util::get_service_config_dir(APP_MANAGER_NAME).join(CONFIG_FILE_NAME);
        let config = Self::load_from_file(&config_file).map_err(|e|{
            error!("load config file {} err {}, use default", config_file.display(), e);
            e
        }).unwrap_or(Self::default());
        info!("final use app manager config: {:?}", toml::to_string(&config));
        config
    }

    fn load_from_file(path: &Path) -> BuckyResult<Self> {
        if !path.exists() {
            return Err(BuckyError::from(BuckyErrorCode::NotFound))
        }

        Ok(toml::from_slice(&std::fs::read(path)?).map_err(|e|{
            let msg = format!("parse app manager config err {}", e);
            error!("{}", &msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?)
    }

    pub fn use_docker(&self) -> bool {
        self.app.use_docker() || self.config.sandbox == SandBoxMode::Docker
    }

    pub fn app_use_docker(&self, id: &DecAppId) -> bool {
        self.app.sandbox.get(id).map(|s|*s == SandBoxMode::Docker).unwrap_or(self.config.sandbox == SandBoxMode::Docker)
    }
}

#[test]
fn test() {
    let config_str = r#"
    [config]
    sandbox = "default"

    [app]
    include = ["9tGpLNnBYrgMNLet1wgFjBZhTUeUgLwML3nFhEvKkLdM"]
    exclude = ["9tGpLNnAAYE9Dd4ooNiSjtP5MeL9CNLf9Rxu6AFEc12M"]
    source = "all"

    [app.sandbox]
    9tGpLNnDpa8deXEk2NaWGccEu4yFQ2DrTZJPLYLT7gj4 = "default"
    9tGpLNnDwJ1nReZqJgWev5eoe23ygViGDC4idnCK1Dy5 = "docker"
    "#;

    let config: AppManagerConfig2 = toml::from_str(config_str).unwrap();

    assert_eq!(config.config.sandbox, SandBoxMode::No);
    assert_eq!(config.app.include[0], DecAppId::from_str("9tGpLNnBYrgMNLet1wgFjBZhTUeUgLwML3nFhEvKkLdM").unwrap());
    assert_eq!(config.app.exclude[0], DecAppId::from_str("9tGpLNnAAYE9Dd4ooNiSjtP5MeL9CNLf9Rxu6AFEc12M").unwrap());
    assert_eq!(config.app.source, AppSource::All);
    for (id, mode) in &config.app.sandbox {
        println!("{} => {}", id, mode)
    }
}