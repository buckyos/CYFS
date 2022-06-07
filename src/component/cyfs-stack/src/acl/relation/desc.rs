use cyfs_base::*;
use cyfs_lib::*;

use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum AclRelationWho {
    My,
    Source, // 只对in操作有效
    Target, // 只对out操作有效
    Your,  // 对in和out操作有效: in：your=source; out: your=target 
}

impl FromStr for AclRelationWho {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "my" => Self::My,
            "source" => Self::Source,
            "target" => Self::Target,
            "your" => Self::Your,
            _ => {
                let msg = format!("invalid acl relation who: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum AclRelationWhat {
    Ood,
    Zone,
    Device,
    Friend,
}

impl FromStr for AclRelationWhat {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "ood" => Self::Ood,
            "zone" => Self::Zone,
            "device" => Self::Device,
            "friend" => Self::Friend,
            _ => {
                let msg = format!("invalid acl relation what: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum AclRelationCategory {
    Device,
    Object,
}

impl FromStr for AclRelationCategory {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "device" => Self::Device,
            "object" | "obj" => Self::Object,
            _ => {
                let msg = format!("invalid acl relation category: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct AclRelationDescription {
    pub who: AclRelationWho,
    pub what: AclRelationWhat,
    pub category: AclRelationCategory,
}

impl AclRelationDescription {
    pub fn load(s: &str) -> BuckyResult<Self> {
        let mut parts: Vec<&str> = s.trim().split("-").collect();
        if parts.is_empty() {
            let msg = format!("invalid acl relation: {}", s);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let category = AclRelationCategory::from_str(parts.pop().unwrap())?;
        let what;
        let who;
        if parts.len() == 2 {
            what = AclRelationWhat::from_str(parts.pop().unwrap())?;
            who = AclRelationWho::from_str(parts.pop().unwrap())?;
        } else if parts.len() == 1 {
            what = AclRelationWhat::Device;
            who = AclRelationWho::from_str(parts.pop().unwrap())?;
        } else {
            let msg = format!("invalid acl relation: {}", s);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(AclRelationDescription {
            who,
            what,
            category,
        })
    }

    pub fn check_valid(&self, action: &AclAction) -> BuckyResult<()> {
        if self.who == AclRelationWho::My || self.who == AclRelationWho::Your {
            return Ok(());
        }

        match action.direction {
            AclDirection::In => {
                if self.who != AclRelationWho::Source {
                    let msg = format!("acl group relation who=Target only valid on direction=Out!");
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }
            }
            AclDirection::Out => {
                if self.who != AclRelationWho::Target {
                    let msg = format!("acl group relation who=Source only valid on direction=In!");
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }
            }
            AclDirection::Any => {
                let msg = format!("acl group relation who=Target or who=Source not valid on direction=*! please use who=Your instead!");
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        }

        Ok(())
    }
}
