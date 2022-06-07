use super::asset::InstallTarget;
use cyfs_base::{BuckyError, BuckyResult};

const SYSTEM_CONFIG_TEMPLATE: &str = r#"
[device]
target = "${target}"
config_desc = "${config_repo}"
version = "nightly"

[[repository]]
type = "${repo_type}"
local_store = "${repo_store}"
"#;

pub struct SystemConfigGen {
    target: InstallTarget,
}

impl SystemConfigGen {
    pub fn new(target: &InstallTarget) -> Self {
        Self {
            target: target.to_owned(),
        }
    }

    pub fn gen(&self) -> BuckyResult<()> {
        let platform_target = Self::get_platform();

        // 如果是solo模式，那么config也使用本地配置
        let config_repo = match self.target {
            InstallTarget::Solo => "local",
            _ => "cyfs_repo",
        };

        // 如果是vood/solo模式，那么使用local_repo
        let repo = match self.target {
            InstallTarget::VOOD | InstallTarget::Solo => "local",
            _ => "named_data",
        };

        let repo_store = cyfs_util::get_cyfs_root_path().join("repo_store");
        let repo_store = repo_store.to_str().unwrap();
        let repo_store = repo_store.replace("\\", "\\\\");
        let value = SYSTEM_CONFIG_TEMPLATE
            .replace("${config_repo}", config_repo)
            .replace("${target}", platform_target)
            .replace("${repo_type}", repo)
            .replace("${repo_store}", &repo_store);

        info!("system-config.toml as follows:\n{}", value);

        return Self::save(value);
    }

    fn get_platform() -> &'static str {
        cyfs_base::get_target()
    }

    fn save(value: String) -> BuckyResult<()> {
        let root = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("ood-daemon");
        if let Err(e) = std::fs::create_dir_all(&root) {
            let msg = format!(
                "create system-config etc dir failed! dir={}, err={}",
                root.display(),
                e
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        let config_file_path = root.join("system-config.toml");
        if config_file_path.exists() {
            warn!(
                "{} already exists! now will overwrite.",
                config_file_path.display()
            );
        }

        if let Err(e) = std::fs::write(&config_file_path, value.as_bytes()) {
            let msg = format!(
                "save system-config failed! file={}, err={}",
                config_file_path.display(),
                e
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        info!(
            "save system-config success! file={}",
            config_file_path.display()
        );

        Ok(())
    }
}
