use cyfs_base::*;


pub const CONFIG_FILE_NAME: &str = "app-manager.toml";

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AppManagerHostMode {
    // default mode, now use docker as sanbox
    Default = 0,

    // native mode for developers, and as default on windows
    Dev = 1,
}


impl ToString for AppManagerHostMode {
    fn to_string(&self) -> String {
        match *self {
            Self::Default => "default",
            Self::Dev => "dev",
        }
        .to_owned()
    }
}

impl std::str::FromStr for AppManagerHostMode {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "default" => Self::Default,
            "dev" => Self::Dev,

            v @ _ => {
                let msg = format!("unknown appmanager host mode type: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
} 

impl Default for AppManagerHostMode {
    fn default() -> Self {
        if cfg!(target_os = "windows") {
            Self::Dev
        } else {
            Self::Default
        }
    }
}
