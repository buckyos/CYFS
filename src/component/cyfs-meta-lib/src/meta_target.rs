use http_types::Url;
use std::str::FromStr;
use cyfs_base::{BuckyError, CyfsChannel};
use log::*;
use std::fmt::Formatter;

const DEV_SERVICE_URL: &str = "http://154.31.50.111:1423";
const TEST_SERVICE_URL: &str = "http://120.24.6.201:1423";

const DEV_SPV_URL: &str = "http://154.31.50.111:3516";
const TEST_SPV_URL: &str = "http://120.24.6.201:3516";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetaMinerTarget {
    // 开发环境
    Dev,

    // 测试环境
    Test,

    // 正式环境
    Formal,

    // other
    Other(Url, Url),
}

impl std::fmt::Display for MetaMinerTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MetaMinerTarget::Dev => {f.write_str("dev")}
            MetaMinerTarget::Test => {f.write_str("test")}
            MetaMinerTarget::Formal => {f.write_str("formal")}
            MetaMinerTarget::Other(miner, spv) => {f.write_fmt(format_args!("meta: {}, spv: {}", miner.to_string(), spv.to_string()))}
        }
    }
}

impl MetaMinerTarget {
    pub fn new(miner: Url, spv: Url) -> Self {
        Self::Other(miner, spv)
    }

    pub fn miner_url(&self) -> String {
        match self {
            Self::Dev => DEV_SERVICE_URL.to_owned(),
            Self::Test => TEST_SERVICE_URL.to_owned(),
            Self::Formal => {
                unimplemented!();
            }
            Self::Other(url, _) => {
                let url = url.to_string();
                if url.ends_with("/") {
                    url.trim_end_matches("/").to_string()
                } else {
                    url
                }
            }
        }
    }

    pub fn spv_url(&self) -> String {
        match self {
            Self::Dev => DEV_SPV_URL.to_owned(),
            Self::Test => TEST_SPV_URL.to_owned(),
            Self::Formal => {
                unimplemented!();
            }
            Self::Other(_, url) => {
                let url = url.to_string();
                if url.ends_with("/") {
                    url.trim_end_matches("/").to_string()
                } else {
                    url
                }
            }
        }
    }
}

impl Default for MetaMinerTarget {
    fn default() -> Self {
        match cyfs_base::get_channel() {
            CyfsChannel::Nightly => Self::Dev,
            CyfsChannel::Beta => Self::Test,
            CyfsChannel::Stable => Self::Formal
        }
    }
}

// 兼容旧的没有spv配置的使用情况
impl FromStr for MetaMinerTarget {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "dev" => Self::Dev,
            "test" => Self::Test,
            "formal" => Self::Formal,
            v @ _ => {
                let mut host = v.to_owned();
                if !host.ends_with("/") {
                    host = host + "/";
                }

                match Url::parse(&host) {
                    Ok(url) => Self::Other(url.clone(), url),
                    Err(e) => {
                        let msg = format!("invalid meta miner target url: {}, {}", host, e);
                        warn!("{}", msg);
                        return Err(BuckyError::from(msg));
                    }
                }
            }
        };

        Ok(ret)
    }
}
