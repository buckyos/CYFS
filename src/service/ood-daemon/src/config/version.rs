use cyfs_base::*;

use std::str::FromStr;

#[derive(Debug)]
pub enum ServiceListVersion {
    Nightly,  // default version
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
            v @ _ => {
                Self::Specific(v.to_owned())
            }
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
