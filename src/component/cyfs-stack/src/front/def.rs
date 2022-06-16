use cyfs_base::*;

use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrontRequestGetMode {
    Default,
    Object,
    Data,
}

impl FrontRequestGetMode {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Default => "default",
            Self::Object => "object",
            Self::Data => "data",
        }
    }
}

impl ToString for FrontRequestGetMode {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl FromStr for FrontRequestGetMode {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "default" => Self::Default,
            "object" => Self::Object,
            "data" => Self::Data,

            _ => {
                // as default action in access get action
                Self::Default
            }
        };

        Ok(ret)
    }
}

#[derive(Clone, Debug, Copy)]
pub enum FrontRequestObjectFormat {
    Default,
    Raw,
    Json,
}

impl FrontRequestObjectFormat {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Default => "default",
            Self::Raw => "raw",
            Self::Json => "json",
        }
    }
}

impl FromStr for FrontRequestObjectFormat {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "default" => Self::Default,
            "raw" => Self::Raw,
            "json" => Self::Json,
            v @ _ => {
                let msg = format!("unknown FrontRequestObjectFormat: {}", v);
                error!("{}", msg);

                Self::Raw
            }
        };

        Ok(ret)
    }
}
