use cyfs_base::*;
use cyfs_noc::*;

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cmp::Ordering;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GlobalStatePathSpecifiedGroup {
    // device/device's owner(as zone id)
    zone: ObjectId,
    access: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
enum GlobalStatePathGroupAccess {
    Specified(GlobalStatePathSpecifiedGroup),
    Default(u32),
}

impl PartialOrd for GlobalStatePathGroupAccess {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match &self {
            Self::Specified(left) => match other {
                Self::Specified(right) => left.zone.partial_cmp(&right.zone),
                Self::Default(_) => Some(Ordering::Less),
            },
            Self::Default(left) => match other {
                Self::Specified(_) => Some(Ordering::Greater),
                Self::Default(right) => left.partial_cmp(&right),
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GlobalStatePathAccessItem {
    // GlobalState path, must end with /
    path: String,

    access: GlobalStatePathGroupAccess,
    // device/device's owner(as zone id)
    // user: Option<ObjectId>,

    // access: u32,
}

impl std::fmt::Display for GlobalStatePathAccessItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.access {
            GlobalStatePathGroupAccess::Default(p) => {
                writeln!(f, "({}, {:o})", self.path, p)
            }
            GlobalStatePathGroupAccess::Specified(s) => {
                writeln!(f, "({}, {}, {:o})", self.path, s.zone, s.access)
            }
        }
    }
}

impl PartialOrd for GlobalStatePathAccessItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.path.partial_cmp(&self.path) {
            Some(Ordering::Equal) | None => self.access.partial_cmp(&other.access),
            ret @ _ => ret,
        }
    }
}

impl Ord for GlobalStatePathAccessItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

/*
impl PartialEq for GlobalStatePathAccessItem {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.access == other.access
    }
}
*/

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStatePathAccessList {
    list: Vec<GlobalStatePathAccessItem>,
}

impl Default for GlobalStatePathAccessList {
    fn default() -> Self {
        Self { list: vec![] }
    }
}

pub struct GlobalStateAccessRequest<'d, 'a, 'b> {
    pub dec: Cow<'d, ObjectId>,
    pub path: Cow<'a, String>,
    pub source: Cow<'b, RequestSourceInfo>,
    pub op_type: RequestOpType,
}

impl<'d, 'a, 'b> std::fmt::Display for GlobalStateAccessRequest<'d, 'a, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "path={}, {}, op={:?}",
            self.path.as_str(),
            self.source,
            self.op_type
        )
    }
}

impl GlobalStatePathAccessList {
    pub fn add(&mut self, item: GlobalStatePathAccessItem) {
        if let Ok(i) = self.list.binary_search(&item) {
            info!("raccess replace item: {} -> {}", self.list[i], item);
            self.list[i] = item;
        } else {
            info!("new raccess item: {}", item);
            self.list.push(item);
            self.list.sort();
        }
    }

    pub fn select<'d, 'a, 'b>(&self, req: GlobalStateAccessRequest<'d, 'a, 'b>) -> BuckyResult<()> {
        assert!(req.path.ends_with('/'));

        for item in &self.list {
            if item.path.starts_with(req.path.as_str()) {
                match &item.access {
                    GlobalStatePathGroupAccess::Default(access) => {
                        let mask = req.source.mask(&req.dec, req.op_type);
                        if mask & access == mask {
                            info!("raccess match item: req={}, access={}", req, item);
                            return Ok(());
                        } else {
                            let msg =
                                format!("raccess reject by item: req={}, access={}", req, item);
                            warn!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                        }
                    }
                    GlobalStatePathGroupAccess::Specified(user) => {
                        if req.source.compare_zone(&user.zone) {
                            let permission: AccessPermission = req.op_type.into();
                            if permission.test(user.access) {
                                info!("raccess match item: req={}, access={}", req, item);
                                return Ok(());
                            } else {
                                let msg =
                                    format!("raccess reject by item: req={}, access={}", req, item);
                                warn!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                            }
                        } else {
                            // Find next path access item
                            continue;
                        }
                    }
                }
            }
        }

        let msg = format!("raccess reject by default: req={}", req);
        warn!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
    }
}
