use cyfs_base::*;
use crate::GlobalStateCategory;

use std::{fmt, str::FromStr};
use std::borrow::Cow;


#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestGlobalStateRoot {
    GlobalRoot(ObjectId),
    DecRoot(ObjectId),
}


impl fmt::Display for RequestGlobalStateRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::GlobalRoot(root) => {
                write!(f, "root:{}", root)
            }
            Self::DecRoot(root) => {
                write!(f, "dec-root:{}", root)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestGlobalStateCommon {
    // default is root-state, can be local-cache
    pub global_state_category: Option<GlobalStateCategory>,

    // root or dec-root object-id
    pub global_state_root: Option<RequestGlobalStateRoot>,

    // target DEC，if is none then equal as source dec-id
    pub dec_id: ObjectId,

    // inernal path of global-state, without the dec-id segment
    pub req_path: Option<String>,
}

impl fmt::Display for RequestGlobalStateCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_string())
    }
}

impl RequestGlobalStateCommon {
    pub fn category(&self) -> GlobalStateCategory {
        match &self.global_state_category {
            Some(v) => *v,
            None => GlobalStateCategory::RootState,
        }
    }

    pub fn req_path(&self) -> Cow<str> {
        match &self.req_path {
            Some(v) => Cow::Borrowed(v.as_str()),
            None => Cow::Borrowed("/"),
        }
    }

    /*
    The first paragraph is optional root-state/local-cache, default root-state
    The second paragraph is optional current/root:{root-id}/dec-root:{dec-root-id}, default is current
    The third paragraph is required target-dec-id
    Fourth paragraph optional global-state-inner-path
    */
    pub fn parse(req_path: &str) -> BuckyResult<Self> {
        let segs: Vec<&str> = req_path
            .trim_start_matches('/')
            .split('/')
            .filter(|seg| !seg.is_empty())
            .collect();

        if segs.is_empty() {
            let msg = format!("invalid request path! {}", req_path);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let mut index = 0;
        let seg = segs[index];
        let global_state_category = match seg {
            "root-state" => {
                index += 1;
                Some(GlobalStateCategory::RootState)
            }
            "local-cache" => {
                index += 1;
                Some(GlobalStateCategory::LocalCache)
            }

            _ => None,
        };

        if index >= segs.len() {
            let msg = format!("invalid request path! {}", req_path);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let seg = segs[index];
        let global_state_root = match seg {
            "current" => {
                index += 1;
                None
            }
            _ if seg.starts_with("root:") => {
                index += 1;

                let id = seg.strip_prefix("root:").unwrap();
                let root = ObjectId::from_str(&id).map_err(|e| {
                    let msg = format!("invalid req_path's root id: {}, {}", seg, e);
                    error!("{msg}");
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

                Some(RequestGlobalStateRoot::GlobalRoot(root))
            }
            _ if seg.starts_with("dec-root:") => {
                index += 1;

                let id = seg.strip_prefix("dec-root:").unwrap();
                let root = ObjectId::from_str(&id).map_err(|e| {
                    let msg = format!("invalid req_path's dec root id: {}, {}", seg, e);
                    error!("{msg}");
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

                Some(RequestGlobalStateRoot::DecRoot(root))
            }
            _ => None,
        };

        if index >= segs.len() {
            let msg = format!("invalid request path param! param={}", req_path);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let seg = segs[index];
        let dec_id = ObjectId::from_str(seg).map_err(|e| {
            let msg = format!("invalid req_path's dec root id: {}, {}", seg, e);
            error!("{msg}");
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;
        index += 1;

        let req_path = if index < segs.len() {
            let path = segs[index..].join("/");
            Some(format!("/{}", path))
        } else {
            None
        };

        Ok(Self {
            global_state_category,
            global_state_root,
            dec_id,
            req_path,
        })
    }

    pub fn format_string(&self) -> String {

        let mut segs: Vec<Cow<str>> = vec![];
        if let Some(v) = &self.global_state_category {
            segs.push(Cow::Borrowed(v.as_str()));
        }

        if let Some(root) = &self.global_state_root {
            segs.push(Cow::Owned(root.to_string()));
        }

        segs.push(Cow::Owned(self.dec_id.to_string()));

        if let Some(path) = &self.req_path {
            segs.push(Cow::Borrowed(path.trim_start_matches('/')));
        }

        format!("/{}", segs.join("/"))
    }
}

impl FromStr for RequestGlobalStateCommon {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let mut root = RequestGlobalStateCommon {
            global_state_category: None,
            global_state_root: None,
            dec_id: ObjectId::default(),
            req_path: Some("/a/b".to_owned()),
        };

        let s = root.format_string();
        println!("{}", s);
        let r = RequestGlobalStateCommon::parse(&s).unwrap();
        assert_eq!(root, r);

        root.global_state_category = Some(GlobalStateCategory::RootState);
        let s = root.format_string();
        println!("{}", s);
        let r = RequestGlobalStateCommon::parse(&s).unwrap();
        assert_eq!(root, r);

        root.global_state_root = Some(RequestGlobalStateRoot::DecRoot(ObjectId::default()));
        let s = root.format_string();
        println!("{}", s);
        let r = RequestGlobalStateCommon::parse(&s).unwrap();
        assert_eq!(root, r);

        root.req_path = None;
        let s = root.format_string();
        println!("{}", s);
        let r = RequestGlobalStateCommon::parse(&s).unwrap();
        assert_eq!(root, r);

        root.req_path = Some("/a/".to_owned());
        let s = root.format_string();
        println!("{}", s);
        let r = RequestGlobalStateCommon::parse(&s).unwrap();
        assert_eq!(root, r);
    }
}