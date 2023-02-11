use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize};
use cyfs_base::*;
use log::*;
use cyfs_util::get_cyfs_root_path;

pub const CONFIG_FILE_NAME: &str = "app-manager.toml";

#[derive(Clone, Serialize, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum SandBoxMode {
    // no sandbox
    No,

    // use docker as sandbox
    Docker
}

pub enum RepoMode {
    NamedData,
    Local(PathBuf)
}

impl Default for RepoMode {
    fn default() -> Self {
        Self::NamedData
    }
}

impl FromStr for RepoMode {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "named_data" {
            return Ok(Self::NamedData);
        } else if s == "local" {
            return Ok(Self::Local(get_cyfs_root_path().join("app_repo")));
        }

        let parts: Vec<&str> = s.splitn(2, ":").collect();
        if parts.len() == 2 {
            if parts[0] == "local" {
                return Ok(Self::Local(PathBuf::from(parts[1])));
            } else {
                error!("invalid repo mode: {}", parts[0]);
            }
        }

        let msg = format!("invalid repo str {}", s);
        error!(msg);
        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
    }
}

impl<'de> Deserialize<'de> for RepoMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TStringVisitor::<Self>::new())
    }
}

impl Display for SandBoxMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SandBoxMode::No => {
                f.write_str("no")
            }
            SandBoxMode::Docker => {
                f.write_str("docker")
            }
        }
    }
}

impl FromStr for SandBoxMode {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "no" => Ok(Self::No),
            "docker" => Ok(Self::Docker),
            "default" => Ok(Self::default()),
            v @ _ => {
                let msg = format!("unknown app manager sandbox mode type: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        }
    }
}

impl<'de> Deserialize<'de> for SandBoxMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TStringVisitor::<Self>::new())
    }
}

impl Default for SandBoxMode {
    fn default() -> Self {
        if cfg!(target_os = "windows") {
            Self::No
        } else {
            Self::Docker
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum AppSource {
    All,
    System,
    User
}

impl Display for AppSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppSource::All => f.write_str("all"),
            AppSource::System => f.write_str("system"),
            AppSource::User => f.write_str("user")
        }
    }
}

impl Default for AppSource {
    fn default() -> Self {
        Self::All
    }
}