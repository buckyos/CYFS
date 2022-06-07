use cyfs_base::*;

use std::str::FromStr;
use std::borrow::Cow;


pub enum AclResource {
    Any,
    Glob(globset::GlobMatcher),
}

impl FromStr for AclResource {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = if s == "*" {
            Self::Any
        } else {
            let glob = globset::GlobBuilder::new(s)
                .case_insensitive(true)
                .literal_separator(true)
                .build()
                .map_err(|e| {
                    let msg = format!("parse acl res as glob error! res={}, {}", s, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

            Self::Glob(glob.compile_matcher())
        };
        Ok(ret)
    }
}

impl AclResource {
    pub fn is_match(&self, path: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Glob(v) => {
                if v.is_match(path) {
                    return true;
                }

                // glob模式下， /xxx/** 不能匹配 /xxx，但可以匹配/xxx/，所以我们这里尝试添加末尾/再重试一次
                if !path.ends_with("/") {
                    let path = format!("{}/", path);
                    v.is_match(&path)
                } else {
                    false
                }
            }
        }
    }

    pub fn join(req_path: &Option<String>, object_id: &Option<ObjectId>, inner_path: &Option<String>) -> String {
        let mut parts = vec![];

        if let Some(req_path) = &req_path {
            let v = req_path.trim_start_matches('/').trim_end_matches('/');
            parts.push(Cow::Borrowed(v));
        }

        if let Some(object_id) = object_id {
            let object_id  = object_id.to_string();
            parts.push(Cow::Owned(object_id));
        }

        if let Some(inner_path) = &inner_path {
            let v = inner_path.trim_start_matches('/').trim_end_matches('/');
            parts.push(Cow::Borrowed(v));
        }

        parts.join("/")
    }
}