use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum RouterEventCategory {
    TestEvent,
    ZoneRoleChanged,
}

impl RouterEventCategory {
    pub fn as_str(&self) -> &str {
        match self {
            Self::TestEvent => "test_event",
            Self::ZoneRoleChanged => "zone_role_changed",
        }
    }
}

impl fmt::Display for RouterEventCategory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl FromStr for RouterEventCategory {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        let ret = match s {
            "test_event" => Self::TestEvent,
            "zone_role_changed" => Self::ZoneRoleChanged,

            v @ _ => {
                let msg = format!("unknown router event category: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        Ok(ret)
    }
}

pub trait RouterEventCategoryInfo {
    fn category() -> RouterEventCategory;
}


pub fn extract_router_event_category<P>() -> RouterEventCategory
where
    P: RouterEventCategoryInfo,
{
    P::category()
}
