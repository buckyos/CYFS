use crate::base::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GlobalStatePathSpecifiedGroup {
    // device/device's owner(as zone id), None for any zone
    pub zone: Option<ObjectId>,

    // Choose one between zone and zone_category
    pub zone_category: Option<DeviceZoneCategory>,

    // specified dec, None for any dec
    pub dec: Option<ObjectId>,

    pub access: u8 /*AccessPermissions*/,
}

impl GlobalStatePathSpecifiedGroup {
    pub fn is_empty(&self) -> bool {
        self.zone.is_none() && self.zone_category.is_none() && self.dec.is_none()
    }

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
        let mut is_empty = true;
        if let Some(zone_category) = &self.zone_category {
            if !source.compare_zone_category(*zone_category) {
                return false;
            }
            is_empty = false;
        }

        // FIXME if zone_category exists already, then should try to compare zone field is still exists?
        if let Some(zone) = &self.zone {
            if !source.compare_zone(&zone) {
                return false;
            }
            is_empty = false;
        }

        if let Some(dec) = &self.dec {
            if !source.compare_dec(dec) {
                return false;
            }
            is_empty = false;
        }

        // should not been empty!
        if is_empty {
            warn!("access specified group is empty!");
            return false;
        }

        true
    }
}

impl PartialOrd for GlobalStatePathSpecifiedGroup {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let ret = self.zone_category.partial_cmp(&other.zone_category);
        if ret.is_some() && ret != Some(Ordering::Equal) {
            return ret;
        }

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
    Default(u32 /*AccessString*/),
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

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct GlobalStatePathAccessItem {
    // GlobalState path, must end with /
    pub path: String,

    // Access value
    pub access: GlobalStatePathGroupAccess,
}

impl GlobalStatePathAccessItem {
    pub fn check_valid(&self) -> bool {
        match &self.access {
            GlobalStatePathGroupAccess::Default(_) => {}
            GlobalStatePathGroupAccess::Specified(v) => {
                if v.is_empty() {
                    return false;
                }
            }
        }

        true
    }

    pub fn fix_path(path: impl Into<String> + AsRef<str>) -> String {
        let path = path.as_ref().trim();

        let ret = match path.ends_with("/") {
            true => {
                if path.starts_with('/') {
                    path.into()
                } else {
                    format!("/{}", path.as_ref() as &str)
                }
            }
            false => {
                if path.starts_with('/') {
                    format!("{}/", path.as_ref() as &str)
                } else {
                    format!("/{}/", path.as_ref() as &str)
                }
            }
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
        zone_category: Option<DeviceZoneCategory>,
        dec: Option<ObjectId>,
        access: u8,
    ) -> Self {
        assert!(zone.is_some() || dec.is_some());

        let path = Self::fix_path(path);

        Self {
            path,
            access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
                zone,
                zone_category,
                dec,
                access,
            }),
        }
    }

    pub fn try_fix_path(&mut self) {
        self.path = Self::fix_path(&self.path);
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
                    "({}, zone={:?}, zone_category={:?} dec={:?}, {})",
                    self.path,
                    s.zone,
                    s.zone_category,
                    s.dec.as_ref().map(|id| cyfs_core::dec_id_to_string(id)),
                    AccessPermissions::format_u8(s.access),
                )
            }
        }
    }
}

impl std::fmt::Debug for GlobalStatePathAccessItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self, f)
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;
    use cyfs_core::*;

    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
    struct Temp {
        zone: Option<GlobalStatePathSpecifiedGroup>,
    }

    #[test]
    fn test() {
        let t = GlobalStatePathSpecifiedGroup {
            zone: None,
            zone_category: Some(DeviceZoneCategory::CurrentZone),
            dec: Some(get_system_dec_app().object_id().clone()),
            access: 5,
        };

        let s = serde_json::to_string(&t).unwrap();
        print!("{}", s);
    }
}
