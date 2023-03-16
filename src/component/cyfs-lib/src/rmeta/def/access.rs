use super::path::GlobalStatePathHelper;
use crate::base::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::borrow::Cow;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GlobalStatePathSpecifiedGroup {
    // device/device's owner(as zone id), None for any zone
    pub zone: Option<ObjectId>,

    // Choose one between zone and zone_category
    pub zone_category: Option<DeviceZoneCategory>,

    // specified dec, None for any dec
    pub dec: Option<ObjectId>,

    pub access: u8, /*AccessPermissions*/
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

impl Ord for GlobalStatePathGroupAccess {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl std::fmt::Display for GlobalStatePathGroupAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Default(p) => {
                write!(f, "{}", AccessString::new(*p))
            }
            Self::Specified(s) => {
                write!(
                    f,
                    "zone={:?}, zone_category={:?} dec={:?}, {}",
                    s.zone,
                    s.zone_category,
                    s.dec.as_ref().map(|id| cyfs_core::dec_id_to_string(id)),
                    AccessPermissions::format_u8(s.access),
                )
            }
        }
    }
}

impl GlobalStatePathGroupAccess {
    pub fn check_valid(&self) -> bool {
        match &self {
            Self::Default(_) => {}
            Self::Specified(v) => {
                if v.is_empty() {
                    return false;
                }
            }
        }

        true
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
        self.access.check_valid()
    }

    pub fn new(path: &str, access: u32) -> Self {
        let path = GlobalStatePathHelper::fix_path(path).to_string();

        Self {
            path,
            access: GlobalStatePathGroupAccess::Default(access),
        }
    }

    pub fn new_group(
        path: &str,
        zone: Option<ObjectId>,
        zone_category: Option<DeviceZoneCategory>,
        dec: Option<ObjectId>,
        access: u8,
    ) -> Self {
        assert!(zone.is_some() || dec.is_some());

        let path = GlobalStatePathHelper::fix_path(path).to_string();

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
        self.path = GlobalStatePathHelper::fix_path(&self.path).to_string();
    }
}

impl std::fmt::Display for GlobalStatePathAccessItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.path, self.access)
    }
}

impl std::fmt::Debug for GlobalStatePathAccessItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self, f)
    }
}

impl PartialOrd for GlobalStatePathAccessItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match GlobalStatePathHelper::compare_path(&self.path, &other.path) {
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

pub struct GlobalStateAccessRequest<'d, 'a, 'b> {
    pub dec: Cow<'d, ObjectId>,
    pub path: Cow<'a, str>,
    pub source: Cow<'b, RequestSourceInfo>,
    pub permissions: AccessPermissions,
}

impl<'d, 'a, 'b> std::fmt::Display for GlobalStateAccessRequest<'d, 'a, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "path={}, {}, permissions={}",
            self.path,
            self.source,
            self.permissions.as_str()
        )
    }
}


#[cfg(test)]
mod test {
    use super::*;
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
            dec: Some(get_system_dec_app().clone()),
            access: 5,
        };

        let s = serde_json::to_string(&t).unwrap();
        print!("{}", s);
    }
}
