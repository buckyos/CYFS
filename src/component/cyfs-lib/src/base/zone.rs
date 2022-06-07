use cyfs_base::*;

use std::fmt;
use std::str::FromStr;


#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ZoneDirection {
    LocalToLocal,
    LocalToRemote,
    RemoteToLocal,
}

impl ZoneDirection {
    fn as_string(&self) -> &str {
        match *self {
            ZoneDirection::LocalToLocal => "local_to_local",
            ZoneDirection::LocalToRemote => "local_to_remote",
            ZoneDirection::RemoteToLocal => "remote_to_local",
        }
    }
}


impl fmt::Display for ZoneDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_string(), f)
    }
}


impl FromStr for ZoneDirection {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "local_to_local" => ZoneDirection::LocalToLocal,
            "local_to_remote" => ZoneDirection::LocalToRemote,
            "remote_to_local" => ZoneDirection::RemoteToLocal,
            v @ _ => {
                let msg = format!("unknown zone direction: {}", v);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        Ok(ret)
    }
}


