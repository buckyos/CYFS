use cyfs_base::*;
use std::str::FromStr;

// the rule of current device in owner'zone
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, RawEncode, RawDecode)]
pub enum ZoneRole {
    ActiveOOD = 0,
    StandbyOOD = 1,
    ReservedOOD = 2,
    Device = 3, // none ood device
}

impl ZoneRole {
    pub fn is_ood_device(&self) -> bool {
        match &self {
            Self::Device => false,
            _ => true,
        }
    }

    pub fn is_active_ood(&self) -> bool {
        match &self {
            Self::ActiveOOD => true,
            _ => false,
        }
    }

    pub fn as_str(&self) -> &str {
        match &self {
            Self::ActiveOOD => "active-ood",
            Self::StandbyOOD => "standby-ood",
            Self::ReservedOOD => "reserved-ood",
            Self::Device => "device",
        }
    }
}

impl std::fmt::Display for ZoneRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}


impl FromStr for ZoneRole {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "active-ood" => Self::ActiveOOD,
            "standby-ood" => Self::StandbyOOD,
            "reserved-ood" => Self::ReservedOOD,
            "device" => Self::Device,
            v @ _ => {
                let msg = format!("unknown ZoneRole: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        Ok(ret)
    }
}
