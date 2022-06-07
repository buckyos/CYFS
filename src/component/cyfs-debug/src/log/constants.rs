use cyfs_base::{BuckyError, BuckyResult};
use std::{fmt::{Display, self}, str::FromStr};
use log::{LevelFilter, Level};

use flexi_logger::Duplicate;
use serde::{Serialize, Deserialize};

#[repr(usize)]
#[derive(Copy, Eq, PartialEq, PartialOrd, Ord, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    Off = 0,
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self {
        #[cfg(debug_assertions)]
        {Self::Debug}

        #[cfg(not(debug_assertions))]
        {Self::Info}
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let level = match *self {
            Self::Off => "off",
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self:: Warn => "warn",
            Self::Error => "error",
        };
        write!(f, "{}", level)
    }
}
impl FromStr for LogLevel {
    type Err = BuckyError;

    /// Parse a string representation of an IPv4 address.
    fn from_str(level: &str) -> BuckyResult<LogLevel> {
        use LogLevel::*;

        let ret = match level {
            "off" => Off,
            "trace" => Trace,
            "debug" => Debug,
            "info" => Info,
            "warn" => Warn,
            "error" => Error,
            v @ _ => {
                println!("invalid log level: {}", v);
                Off
            }
        };

        Ok(ret)
    }
}


impl Into<Duplicate> for LogLevel {
    fn into(self) -> Duplicate {

        match self {
            Self::Trace => Duplicate::Trace,
            Self::Debug => Duplicate::Debug,
            Self::Info => Duplicate::Info,
            Self:: Warn => Duplicate::Warn,
            Self::Error => Duplicate::Error,
            Self::Off => Duplicate::None,
        }
    }
}

impl Into<LevelFilter> for LogLevel {
    fn into(self) -> LevelFilter {

        match self {
            Self::Trace => LevelFilter::Trace,
            Self::Debug => LevelFilter::Debug,
            Self::Info => LevelFilter::Info,
            Self:: Warn => LevelFilter::Warn,
            Self::Error => LevelFilter::Error,
            Self::Off => LevelFilter::Off,
        }
    }
}

impl From<LevelFilter> for LogLevel {
    fn from(v: LevelFilter) -> Self {
        match v {
            LevelFilter::Trace => LogLevel::Trace,
            LevelFilter::Debug => LogLevel::Debug,
            LevelFilter::Info => LogLevel::Info,
            LevelFilter:: Warn => LogLevel::Warn,
            LevelFilter::Error => LogLevel::Error,
            LevelFilter::Off => LogLevel::Off,
        }
    }
}


impl From<Level> for LogLevel {
    fn from(v: Level) -> Self {

        match v {
            Level::Trace => Self::Trace,
            Level::Debug => Self::Debug,
            Level::Info => Self::Info,
            Level::Warn => Self::Warn,
            Level::Error => Self::Error,
        }
    }
}