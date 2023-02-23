use cyfs_base::*;

use std::str::FromStr;

#[derive(Debug)]
pub enum ServiceListVersion {
    Nightly,          // default version
    Specific(String), // user confined
}

// FIXME 默认使用nightly，以后会切换成stable
impl Default for ServiceListVersion {
    fn default() -> Self {
        Self::Nightly
    }
}

impl ToString for ServiceListVersion {
    fn to_string(&self) -> String {
        match *self {
            Self::Nightly => "nightly",
            Self::Specific(ref v) => v.as_str(),
        }
        .to_owned()
    }
}

impl FromStr for ServiceListVersion {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        let ret = match s.trim() {
            "nightly" => Self::Nightly,
            v @ _ => Self::Specific(v.to_owned()),
        };

        Ok(ret)
    }
}

// 版本列表定义
#[derive(Debug)]
pub enum ServiceVersion {
    Default,          // use the version config in service list
    Specific(String), // semver, * as the newest version
}

impl ServiceVersion {
    pub fn is_default(&self) -> bool {
        match self {
            Self::Default => true,
            _ => false,
        }
    }
}

impl Default for ServiceVersion {
    fn default() -> Self {
        Self::Default
    }
}

impl ToString for ServiceVersion {
    fn to_string(&self) -> String {
        match *self {
            Self::Default => "default",
            Self::Specific(ref v) => v.as_str(),
        }
        .to_owned()
    }
}

impl FromStr for ServiceVersion {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        let ret = match s.trim() {
            "default" => Self::Default,
            v @ _ => {
                let _req_version = semver::VersionReq::parse(v).map_err(|e| {
                    let msg = format!("invalid semver request string! value={}, {}", v, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

                Self::Specific(v.to_owned())
            }
        };

        Ok(ret)
    }
}

pub struct SemVerEpochCheck;

impl SemVerEpochCheck {
    pub fn get_semver_epoch_patch_version() -> u64 {
        match cyfs_base::get_channel() {
            CyfsChannel::Nightly => 719,
            CyfsChannel::Beta => 75,
            CyfsChannel::Stable => 0,
        }
    }

    pub fn check_version_with_semver_epoch(semver: &str) -> BuckyResult<()> {
        let version = semver::Version::parse(semver).map_err(|e| {
            let msg = format!("invalid semver string! value={}, {}", semver, e,);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        if version.patch < Self::get_semver_epoch_patch_version() {
            let msg = format!(
                "version that does not support semver! version={}, epoch patch={}",
                semver,
                Self::get_semver_epoch_patch_version(),
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
        }

        Ok(())
    }
}
