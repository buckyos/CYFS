use std::path::{Path, PathBuf};
use std::sync::Mutex;

//use crate::{PeerDesc, BuckyResult};

pub const CFYS_ROOT_NAME: &str = "cyfs";

fn default_cyfs_root_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(&format!("C:\\{}", CFYS_ROOT_NAME))
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "ios", target_os = "macos"))]
    {
        PathBuf::from(&format!("/{}", CFYS_ROOT_NAME))
    }

    #[cfg(target_arch = "wasm32")]
    {
        PathBuf::new()
    }
}

lazy_static::lazy_static! {
    pub static ref CYFS_ROOT: Mutex<PathBuf> = {
        let path = default_cyfs_root_path();
        if !path.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&path) {
                error!(
                    "create cyfs root dir failed! dir={}, err={}",
                    path.display(),
                    e
                );
            }
        }

        info!("cyfs root: {}", path.display());
        Mutex::new(PathBuf::from(path))
    };
}

// 初始化时候调用一次
pub fn bind_cyfs_root_path(root_path: impl Into<PathBuf>) {
    let root_path: PathBuf = root_path.into();
    info!("change cyfs root path to: {}", root_path.display());

    *CYFS_ROOT.lock().unwrap() = root_path;
}

pub fn get_cyfs_root_path() -> PathBuf {
    CYFS_ROOT.lock().unwrap().clone()
}

pub fn get_temp_path() -> PathBuf {
    let tmp = get_cyfs_root_path().join("tmp");
    if let Err(e) = std::fs::create_dir_all(&tmp) {
        error!("create tmp dir failed! dir={}, err={}", tmp.display(), e);
    }

    tmp
}

pub fn get_log_dir(service_name: &str) -> PathBuf {
    return get_cyfs_root_path().join("log").join(service_name);
}

pub fn get_app_log_dir(app_name: &str) -> PathBuf {
    let mut path = get_cyfs_root_path();
    path.push("log");
    path.push("app");
    path.push(app_name);
    path
}

pub fn get_app_dir(app_id: &str) -> PathBuf {
    get_cyfs_root_path().join("app").join(app_id)
}

pub fn get_app_web_dir(app_id: &str) -> PathBuf {
    get_cyfs_root_path().join("app").join("web").join(app_id)
}

pub fn get_app_acl_dir(app_id: &str) -> PathBuf {
    get_cyfs_root_path().join("app").join("acl").join(app_id)
}

pub fn get_app_dep_dir(app_id: &str, version: &str) -> PathBuf {
    get_cyfs_root_path()
        .join("app")
        .join("dependent")
        .join(app_id)
        .join(version)
}

pub fn get_app_dockerfile_dir(app_id: &str) -> PathBuf {
    get_cyfs_root_path()
        .join("app")
        .join("dockerfile")
        .join(app_id)
}

pub fn get_app_data_dir(app_name: &str) -> PathBuf {
    let mut base_dir = get_cyfs_root_path();
    base_dir.push("data");
    base_dir.push("app");
    base_dir.push(app_name);
    if let Err(e) = std::fs::create_dir_all(&base_dir) {
        error!(
            "create app data dir failed! dir={}, err={}",
            base_dir.display(),
            e
        );
    }

    base_dir
}

pub fn get_app_data_dir_ex(app_name: &str, base: &Path) -> PathBuf {
    let mut base_dir = base.to_owned();
    base_dir.push("data");
    base_dir.push("app");
    base_dir.push(app_name);
    base_dir
}

// {CFYS_ROOT_NAME}/etc/{service_name}/
pub fn get_service_config_dir(service_name: &str) -> PathBuf {
    let mut base_dir = get_cyfs_root_path();
    base_dir.push("etc");
    base_dir.push(service_name);
    base_dir
}

pub fn get_service_data_dir(service_name: &str) -> PathBuf {
    let mut base_dir = get_cyfs_root_path();
    base_dir.push("data");
    base_dir.push(service_name);
    if let Err(e) = std::fs::create_dir_all(&base_dir) {
        error!(
            "create service data dir failed! dir={}, err={}",
            base_dir.display(),
            e
        );
    }
    base_dir
}

pub fn get_named_data_root(isolate: &str) -> PathBuf {
    let mut base_dir = get_cyfs_root_path();
    base_dir.push("data");
    if isolate.len() > 0 {
        base_dir.push(isolate)
    }
    base_dir.push("named-data-cache");

    if !base_dir.is_dir() {
        if let Err(e) = std::fs::create_dir_all(&base_dir) {
            error!(
                "create bdt storage dir failed! dir={}, err={}",
                base_dir.display(),
                e
            );
        } else {
            info!(
                "create named-data-cache dir success! {}",
                base_dir.display()
            );
        }
    }

    base_dir
}

/*
pub fn get_bdt_named_data_peer_root(desc_name: &str) -> PathBuf {
    let mut base_dir = get_cyfs_root_path();
    base_dir.push("bdt");
    base_dir.push("named_data_cache");
    base_dir.push("chunk");
    base_dir.push(desc_name);

    if let Err(e) = std::fs::create_dir_all(&base_dir) {
        error!(
            "create bdt storage dir failed! dir={}, err={}",
            base_dir.display(),
            e
        );
    }

    base_dir
}
*/
