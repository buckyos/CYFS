use std::str::FromStr;

use cyfs_base::{BuckyError, BuckyErrorCode};

pub enum ServiceMode {
    Backup,
    Restore,
    Interactive,
}

impl ServiceMode {
    pub fn as_str(&self) -> &'static str {
        match *self {
            Self::Backup => "backup",
            Self::Restore => "restore",
            Self::Interactive => "interactive",
        }
    }

    pub fn str_list() -> String {
        let list: Vec<&str> = [Self::Backup, Self::Restore, Self::Interactive]
            .into_iter()
            .map(|v| v.as_str())
            .collect();
        list.join(" ,")
    }
}

impl FromStr for ServiceMode {
    type Err = BuckyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "backup" => Self::Backup,
            "restore" => Self::Restore,
            "interactive" => Self::Interactive,
            _ => {
                let msg = format!("unsupported mode: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
            }
        })
    }
}
