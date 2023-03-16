use std::path::{Path, PathBuf};

pub const CFYS_ROOT_NAME: &str = "cyfs";

pub fn default_cyfs_root_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(&format!("C:\\{}", CFYS_ROOT_NAME))
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "ios"))]
    {
        PathBuf::from(&format!("/{}", CFYS_ROOT_NAME))
    }

    #[cfg(target_os = "macos")]
    {
        match dirs::data_dir() {
            Some(dir) => {
                let root = dir.join(&format!("../{}", CFYS_ROOT_NAME));
                if root.is_dir() {
                    root.canonicalize().unwrap()
                } else {
                    root
                }
            }
            None => {
                error!("get user dir failed!");
                PathBuf::from(&format!("/{}", CFYS_ROOT_NAME))
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        PathBuf::new()
    }
}

static CYFS_ROOT: once_cell::sync::OnceCell<PathBuf> = once_cell::sync::OnceCell::new();

// 初始化时候调用一次
pub fn bind_cyfs_root_path(root_path: impl Into<PathBuf>) {
    let root_path: PathBuf = root_path.into();
    println!("bind cyfs_root dir: {}", root_path.display());

    match CYFS_ROOT.set(root_path.clone()) {
        Ok(_) => {
            info!("change cyfs root path to: {}", root_path.display());
            if !root_path.is_dir() {
                if let Err(e) = std::fs::create_dir_all(&root_path) {
                    error!(
                        "create cyfs root dir failed! dir={}, err={}",
                        root_path.display(),
                        e
                    );
                }
            }
        }
        Err(_) => {
            unreachable!("change cyfs root after been inited! {}", root_path.display());
        }
    }
}

pub fn get_cyfs_root_path_ref() -> &'static Path {
    CYFS_ROOT.get_or_init(|| {
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
        path
    })
}

pub fn get_cyfs_root_path() -> PathBuf {
    get_cyfs_root_path_ref().to_owned()
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

pub fn get_app_dep_dir(app_id: &str) -> PathBuf {
    get_cyfs_root_path()
        .join("app")
        .join("dependent")
        .join(app_id)
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
