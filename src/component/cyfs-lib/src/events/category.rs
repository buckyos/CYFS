use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum RouterEventCategory {
    TestEvent,
}

impl fmt::Display for RouterEventCategory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match &*self {
            Self::TestEvent => "test_event",
        };

        fmt::Display::fmt(s, f)
    }
}

impl FromStr for RouterEventCategory {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        let ret = match s {
            "test_event" => Self::TestEvent,

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
