use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, CyfsChannel};

use rust_embed::RustEmbed;
#[cfg(not(windows))]
use std::path::PathBuf;
use std::str::FromStr;
use std::{fmt::Display, path::Path};

#[derive(Clone, Eq, PartialEq)]
pub enum InstallTarget {
    Default,
    Synology,
    VOOD,
    Solo,
}

impl InstallTarget {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Default => "default",
            Self::Synology => "synology",
            Self::VOOD => "vood",
            Self::Solo => "solo",
        }
    }
}

impl Display for InstallTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for InstallTarget {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "default" => Self::Default,
            "synology" => Self::Synology,
            "vood" => Self::VOOD,
            "solo" => Self::Solo,
            _ => {
                let msg = format!("unknown install target: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(RustEmbed)]
#[folder = "res/"]
struct Asset;

pub struct OODAsset {
    target: InstallTarget,
}

impl OODAsset {
    pub fn new(target: &InstallTarget) -> Self {
        Self {
            target: target.to_owned(),
        }
    }

    pub fn extract(&self, no_cyfs_repo: bool, no_app_repo: bool) -> BuckyResult<()> {
        if !no_cyfs_repo {
            if let Err(e) = self.extract_cyfs_repo() {
                return Err(e);
            }
        }

        if let Err(e) = self.extract_gateway() {
            return Err(e);
        }

        //if let Err(e) = self.extract_acc_service() {
        //    return Err(e);
        //}

        if !no_app_repo {
            if let Err(e) = self.extract_app_repo() {
                return Err(e);
            }
        }
        let _ = self.extract_debug_config();
        let _ = self.extract_acl_config();
        // 按照司司的说法，只有ood才需要设置pn。默认的pn desc在ood安装的时候释放出来
        let _ = self.extract_pn_desc();

        if self.target == InstallTarget::Default {
            #[cfg(unix)]
            if let Err(e) = self.extract_service_script() {
                return Err(e);
            }
        }

        Ok(())
    }

    fn extract_acl_config(&self) -> BuckyResult<()> {
        let root = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("acl")
            .join("acl.toml");
        self.extract_from_asset(&root, "acl.toml")
    }

    fn extract_pn_desc(&self) -> BuckyResult<()> {
        let root = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("desc")
            .join("pn.desc");
        self.extract_from_asset(&root, "pn.desc")
    }

    fn extract_debug_config(&self) -> BuckyResult<()> {
        let root = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("debug.toml");

        //if !root.is_file() {
        self.extract_from_asset(&root, "debug.toml")
        //} else {
        //   warn!("{} already exists!", root);
        //   Ok(())
        //}
    }

    fn extract_cyfs_repo(&self) -> BuckyResult<()> {
        let root = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("desc")
            .join("cyfs_repo.desc");

        self.extract_from_asset(&root, "cyfs_repo.desc")
    }

    fn extract_gateway(&self) -> BuckyResult<()> {
        let root = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("gateway")
            .join("gateway.toml");

        self.extract_from_asset(&root, "gateway.toml")
    }

    fn extract_app_repo(&self) -> BuckyResult<()> {
        let root = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("desc")
            .join("app_repo.desc");

        self.extract_from_asset(&root, "app_repo.desc")
    }

    fn extract_from_asset(&self, dest_path: &Path, asset_path: &str) -> BuckyResult<()> {
        if let Err(e) = std::fs::create_dir_all(dest_path.parent().unwrap()) {
            let msg = format!(
                "create dir error! dir={}, err={}",
                dest_path.parent().unwrap().display(),
                e
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        // solo模式提取资源暂时和default模式一致
        let target_dir = match self.target {
            InstallTarget::Solo => InstallTarget::Default,
            _ => self.target.clone(),
        };

        // 检查两次，如果{target}/{channel}/{asset}有文件，优先用这里的
        // 如果没有，再用{target}/{asset}取
        let full_path = format!("{}/{}/{}", target_dir.as_str(), cyfs_base::get_channel().to_string(), asset_path);
        let ret = Asset::get(&full_path).or_else(||{
            let full_path = format!("{}/{}", target_dir.as_str(), asset_path);
            Asset::get(&full_path)
        }).unwrap();

        if let Err(e) = std::fs::write(dest_path.clone(), ret.data) {
            let msg = format!(
                "extract file error! file={}, err={}",
                dest_path.display(),
                e
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        info!("extract file success! file={}", dest_path.display());

        Ok(())
    }

    #[cfg(not(windows))]
    fn extract_service_script(&self) -> BuckyResult<()> {
        let file_path = PathBuf::from("/etc/init.d").join("ood-daemon");

        self.extract_from_asset(&file_path, "ood-daemon.sh")?;

        use std::os::unix::fs::PermissionsExt;
        let permissions = PermissionsExt::from_mode(0o777);

        if let Err(e) = std::fs::set_permissions(&file_path, permissions) {
            let msg = format!(
                "change file permissions! file={}, err={}",
                file_path.display(),
                e
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        Ok(())
    }
}
