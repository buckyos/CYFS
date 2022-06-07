use cyfs_base::*;

use std::str::FromStr;

// 版本列表定义
#[derive(Debug)]
pub enum ServiceListVersion {
    Nightly,
    Latest,
    Stable,
    Specific(String),
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
            Self::Latest => "latest",
            Self::Stable => "stable",
            Self::Specific(ref v) => v.as_str(),
        }
        .to_owned()
    }
}

impl FromStr for ServiceListVersion {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        let ret = match s {
            "nightly" => Self::Nightly,
            "latest" => Self::Latest,
            "stable" => Self::Stable,
            _ => Self::Specific(s.to_owned()),
        };

        Ok(ret)
    }
}
