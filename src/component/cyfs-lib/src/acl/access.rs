use cyfs_base::*;

use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AclAccess {
    Accept = 0,
    Reject = 1,
    Drop = 2,
    Pass = 3,
}

impl AclAccess {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Accept => "accept",
            Self::Reject => "reject",
            Self::Drop => "drop",
            Self::Pass => "pass",
        }
    }
}

impl ToString for AclAccess {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl FromStr for AclAccess {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "accept" => Self::Accept,
            "reject" => Self::Reject,
            "drop" => Self::Drop,
            "pass" => Self::Pass,

            _ => {
                let msg = format!("unknown acl access: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}
