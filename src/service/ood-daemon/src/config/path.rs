use lazy_static::lazy_static;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub struct Paths {
    pub system_config: PathBuf,
    pub device_config: PathBuf,
    pub service_root: PathBuf,
    pub repo_cache_root: PathBuf,
}

impl Paths {
    pub fn new() -> Paths {
        let root_path = ::cyfs_util::get_cyfs_root_path();
        let system_config = root_path.join("etc/ood-daemon/system-config.toml");

        let device_config = root_path.join("etc/ood-daemon/device-config.toml");
        let service_root = root_path.join("services");
        let repo_cache_root = cyfs_util::get_temp_path().join("repo");

        Paths {
            system_config,
            device_config,
            service_root,
            repo_cache_root,
        }
    }
}

lazy_static! {
    pub static ref PATHS: Arc<Paths> = Arc::new(Paths::new());
}
