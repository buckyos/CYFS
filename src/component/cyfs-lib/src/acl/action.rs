use cyfs_base::*;

use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AclOperationCategory {
    Both,
    Read,
    Write,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AclAction {
    Accept = 0,
    Reject = 1,
}

impl AclAction {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Accept => "accept",
            Self::Reject => "reject",
        }
    }
}

impl ToString for AclAction {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl FromStr for AclAction {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "accept" => Self::Accept,
            "reject" => Self::Reject,

            _ => {
                let msg = format!("unknown acl action: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}
