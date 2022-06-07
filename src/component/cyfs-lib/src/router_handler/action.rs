use cyfs_base::*;

use serde_json::{Map, Value};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouterHandlerAction {
    Default,
    Response,

    Reject,
    Drop,

    Pass,
}

impl RouterHandlerAction {
    pub fn to_error_code(&self) -> BuckyErrorCode {
        match *self {
            RouterHandlerAction::Reject => BuckyErrorCode::Reject,
            RouterHandlerAction::Drop => BuckyErrorCode::Ignored,

            _ => BuckyErrorCode::Ok,
        }
    }

    pub fn is_action_error(e: &BuckyError) -> bool {
        Self::is_action_error_code(&e.code())
    }

    pub fn is_action_error_code(code: &BuckyErrorCode) -> bool {
        match code {
            BuckyErrorCode::Reject | BuckyErrorCode::Ignored => true,
            _ => false,
        }
    }
}

impl fmt::Display for RouterHandlerAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match &*self {
            Self::Default => "Default",
            Self::Response => "Response",

            Self::Reject => "Reject",
            Self::Drop => "Drop",

            Self::Pass => "Pass",
        };

        fmt::Display::fmt(s, f)
    }
}

impl FromStr for RouterHandlerAction {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        let ret = match s {
            "Default" => Self::Default,
            "Response" => Self::Response,

            "Reject" => Self::Reject,
            "Drop" => Self::Drop,

            "Pass" => Self::Pass,
            v @ _ => {
                let msg = format!("unknown router handler action: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        Ok(ret)
    }
}

impl JsonCodec<RouterHandlerAction> for RouterHandlerAction {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        obj.insert("action".to_owned(), Value::String(self.to_string()));

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let action = obj.get("action").ok_or_else(|| {
            error!("action field missed! {:?}", obj);
            BuckyError::from(BuckyErrorCode::InvalidFormat)
        })?;

        let action = action.as_str().unwrap_or("");
        let ret = Self::from_str(action)?;

        Ok(ret)
    }
}
