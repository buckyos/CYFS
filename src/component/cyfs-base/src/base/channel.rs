use super::error::{BuckyError, BuckyResult};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CyfsChannel {
    Nightly,
    Beta,
    Stable,
}

impl FromStr for CyfsChannel {
    type Err = BuckyError;

    fn from_str(str: &str) -> BuckyResult<Self> {
        let ret = match str {
            "nightly" => CyfsChannel::Nightly,
            "beta" => CyfsChannel::Beta,
            "stable" => CyfsChannel::Stable,
            _ => {
                log::warn!("unknown channel name {}, use default nightly channel", str);
                CyfsChannel::Nightly
            }
        };

        Ok(ret)
    }
}

impl Display for CyfsChannel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CyfsChannel::Nightly => write!(f, "nightly"),
            CyfsChannel::Beta => write!(f, "beta"),
            CyfsChannel::Stable => write!(f, "stable"),
        }
    }
}

impl CyfsChannel {
    fn get_ver(&self) -> u8 {
        match self {
            CyfsChannel::Nightly => 0,
            CyfsChannel::Beta => 1,
            CyfsChannel::Stable => 2,
        }
    }
}

pub fn get_version() -> &'static str {
    &VERSION
}

pub fn get_channel() -> &'static CyfsChannel {
    &CHANNEL
}

pub fn get_target() -> &'static str {
    &TARGET
}

fn get_version_impl() -> String {
    let channel_ver = get_channel().get_ver();
    format!("1.1.{}.{}-{} ({})", channel_ver, env!("VERSION"), get_channel(), env!("BUILDDATE"))
}

fn get_channel_impl() -> CyfsChannel {
    let channel_str = match std::env::var("CYFS_CHANNEL") {
        Ok(channel) => {
            info!("got channel config from CYFS_CHANNEL env: channel={}", channel);
            channel
        }
        Err(_) => {
            let channel = env!("CHANNEL").to_owned();
            info!("use default channel config: channel={}", channel);
            channel
        }
    };
    
    CyfsChannel::from_str(channel_str.as_str()).unwrap()
}

lazy_static::lazy_static! {
    static ref CHANNEL: CyfsChannel = get_channel_impl();
    static ref VERSION: String = get_version_impl();
    static ref TARGET: &'static str = env!("TARGET");
}
