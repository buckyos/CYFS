use cyfs_base::*;
use crate::access::*;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;


#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GlobalStatePathSpecifiedGroup {
    // device/device's owner(as zone id), None for any zone
    pub zone: Option<ObjectId>,

    // specified dec, None for any dec
    pub dec: Option<ObjectId>,

    pub access: u8,
}

impl GlobalStatePathSpecifiedGroup {
    fn compare_opt_item(left: &Option<ObjectId>, right: &Option<ObjectId>) -> Option<Ordering> {
        match left {
            Some(left) => match right {
                Some(right) => left.partial_cmp(right),
                None => Some(Ordering::Less),
            },
            None => match right {
                Some(_) => Some(Ordering::Greater),
                None => Some(Ordering::Equal),
            },
        }
    }

    pub fn compare(&self, source: &RequestSourceInfo) -> bool {
        if let Some(zone) = &self.zone {
            if !source.compare_zone(&zone) {
                return false;
            }
        }

        if let Some(dec) = &self.dec {
            if !source.compare_dec(dec) {
                return false;
            }
        }

        true
    }
}

impl PartialOrd for GlobalStatePathSpecifiedGroup {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let ret = Self::compare_opt_item(&self.zone, &other.zone);
        if ret.is_some() && ret != Some(Ordering::Equal) {
            return ret;
        }

        Self::compare_opt_item(&self.dec, &other.dec)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum GlobalStatePathGroupAccess {
    Specified(GlobalStatePathSpecifiedGroup),
    Default(u32),
}

impl PartialOrd for GlobalStatePathGroupAccess {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match &self {
            Self::Specified(left) => match other {
                Self::Specified(right) => left.partial_cmp(&right),
                Self::Default(_) => Some(Ordering::Less),
            },
            Self::Default(_left) => match other {
                Self::Specified(_) => Some(Ordering::Greater),
                Self::Default(_right) => Some(Ordering::Equal),
            },
        }
    }
}


#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GlobalStatePathAccessItem {
    // GlobalState path, must end with /
    pub path: String,

    // Access value
    pub access: GlobalStatePathGroupAccess,
}

impl GlobalStatePathAccessItem {
    pub fn fix_path(path: impl Into<String> + AsRef<str>) -> String {
        let path = path.as_ref().trim();

        let ret = match path.ends_with("/") {
            true => path.into(),
            false => format!("{}/", path.as_ref() as &str),
        };

        ret
    }

    pub fn compare_path(left: &String, right: &String) -> Option<Ordering> {
        let len1 = left.len();
        let len2 = right.len();

        if len1 > len2 {
            Some(Ordering::Less)
        } else if len1 < len2 {
            Some(Ordering::Greater)
        } else {
            left.partial_cmp(right)
        }
    }

    pub fn new(path: impl Into<String> + AsRef<str>, access: u32) -> Self {
        let path = Self::fix_path(path);

        Self {
            path,
            access: GlobalStatePathGroupAccess::Default(access),
        }
    }

    pub fn new_group(
        path: impl Into<String> + AsRef<str>,
        zone: Option<ObjectId>,
        dec: Option<ObjectId>,
        access: u8,
    ) -> Self {
        assert!(zone.is_some() || dec.is_some());

        let path = Self::fix_path(path);

        Self {
            path,
            access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
                zone,
                dec,
                access,
            }),
        }
    }
}

impl std::fmt::Display for GlobalStatePathAccessItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.access {
            GlobalStatePathGroupAccess::Default(p) => {
                writeln!(f, "({}, {})", self.path, AccessString::new(*p))
            }
            GlobalStatePathGroupAccess::Specified(s) => {
                writeln!(
                    f,
                    "({}, zone={:?}, dec={:?}, {})",
                    self.path, s.zone, s.dec, AccessPermissions::format_u8(s.access),
                )
            }
        }
    }
}

impl PartialOrd for GlobalStatePathAccessItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match Self::compare_path(&self.path, &other.path) {
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