use super::request::*;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum RouterHandlerCategory {
    PutObject,
    GetObject,

    PostObject,

    SelectObject,
    DeleteObject,

    GetData,
    PutData,
    DeleteData,

    SignObject,
    VerifyObject,

    Acl,
}

impl fmt::Display for RouterHandlerCategory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match &*self {
            Self::PutObject => "put_object",
            Self::GetObject => "get_object",

            Self::PostObject => "post_object",

            Self::SelectObject => "select_object",
            Self::DeleteObject => "delete_object",

            Self::GetData => "get_data",
            Self::PutData => "put_data",
            Self::DeleteData => "delete_data",

            Self::SignObject => "sign_object",
            Self::VerifyObject => "verify_object",

            Self::Acl => "acl",
        };

        fmt::Display::fmt(s, f)
    }
}

impl FromStr for RouterHandlerCategory {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        let ret = match s {
            "put_object" => Self::PutObject,
            "get_object" => Self::GetObject,

            "post_object" => Self::PostObject,

            "select_object" => Self::SelectObject,
            "delete_object" => Self::DeleteObject,

            "get_data" => Self::GetData,
            "put_data" => Self::PutData,
            "delete_data" => Self::DeleteData,

            "sign_object" => Self::SignObject,
            "verify_object" => Self::VerifyObject,

            "acl" => Self::Acl,

            v @ _ => {
                let msg = format!("unknown router handler category: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        Ok(ret)
    }
}

pub trait RouterHandlerCategoryInfo {
    fn category() -> RouterHandlerCategory;
}

impl RouterHandlerCategoryInfo for RouterHandlerPutObjectRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::PutObject
    }
}

impl RouterHandlerCategoryInfo for RouterHandlerGetObjectRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::GetObject
    }
}

impl RouterHandlerCategoryInfo for RouterHandlerPostObjectRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::PostObject
    }
}

impl RouterHandlerCategoryInfo for RouterHandlerSelectObjectRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::SelectObject
    }
}

impl RouterHandlerCategoryInfo for RouterHandlerDeleteObjectRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::DeleteObject
    }
}

impl RouterHandlerCategoryInfo for RouterHandlerPutDataRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::PutData
    }
}

impl RouterHandlerCategoryInfo for RouterHandlerGetDataRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::GetData
    }
}

impl RouterHandlerCategoryInfo for RouterHandlerDeleteDataRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::DeleteData
    }
}


impl RouterHandlerCategoryInfo for RouterHandlerSignObjectRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::SignObject
    }
}

impl RouterHandlerCategoryInfo for RouterHandlerVerifyObjectRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::VerifyObject
    }
}

impl RouterHandlerCategoryInfo for RouterHandlerAclRequest {
    fn category() -> RouterHandlerCategory {
        RouterHandlerCategory::Acl
    }
}

pub fn extract_router_handler_category<P>() -> RouterHandlerCategory
where
    P: RouterHandlerCategoryInfo,
{
    P::category()
}
