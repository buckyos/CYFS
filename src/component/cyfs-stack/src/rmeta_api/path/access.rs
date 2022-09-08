use cyfs_base::*;
use cyfs_lib::*;

use serde::{Deserialize, Serialize};
use std::borrow::Cow;


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
        write!(
            f,
            "path={}, {}, op={:?}",
            self.path.as_str(),
            self.source,
            self.op_type
        )
    }
}

impl GlobalStatePathAccessList {
    pub fn new() -> Self {
        Self { list: vec![] }
    }

    // return true if any changed
    pub fn add(&mut self, item: GlobalStatePathAccessItem) -> bool {
        if let Ok(i) = self.list.binary_search(&item) {
            if item ==  self.list[i] {
                return false;
            }

            info!("raccess replace item: {} -> {}", self.list[i], item);
            self.list[i] = item;
        } else {
            info!("new raccess item: {}", item);
            self.list.push(item);
            self.list.sort();
        }

        true
    }

    pub fn remove(&mut self, item: GlobalStatePathAccessItem) -> Option<GlobalStatePathAccessItem> {
        if let Ok(i) = self.list.binary_search(&item) {
            let item = self.list.remove(i);
            info!("raccess remove item: {}", item);
            Some(item)
        } else {
            info!("raccess remove item but not found: {}", item);
            None
        }
    }

    pub fn clear(&mut self) -> usize {
        if self.list.is_empty() {
            return 0;
        }

        let count = self.list.len();
        self.list.clear();
        count
    }

    pub fn get(&self) -> Vec<GlobalStatePathAccessItem> {
        self.list.clone()
    }

    pub fn check<'d, 'a, 'b>(&self, req: GlobalStateAccessRequest<'d, 'a, 'b>) -> BuckyResult<()> {
        assert!(req.path.ends_with('/'));

        for item in &self.list {
            if req.path.starts_with(item.path.as_str()) {
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
                        if user.compare(&req.source) {
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

#[cfg(test)]
mod test_path_access {
    use super::*;
    use cyfs_core::*;

    fn new_dec(name: &str) -> ObjectId {
        let owner_id = PeopleId::default();
        let dec_id = DecApp::generate_id(owner_id.into(), name);
    
        info!(
            "generage random dec: name={}, dec_id={}",
            name, dec_id
        );
    
        dec_id
    }

    #[test]
    fn test() {
        cyfs_base::init_simple_log("test_path_access", None);

        let owner_dec = new_dec("owner");
        let mut list = GlobalStatePathAccessList::new();
        let default_access = AccessString::default();
        let item = GlobalStatePathAccessItem::new("/a/b", default_access.value());
        list.add(item);

        let item = GlobalStatePathAccessItem::new("/a/b/c/d", 0);
        list.add(item);

        let mut access = AccessString::default();
        access.set_group_permissions(AccessGroup::OthersDec, AccessPermissions::ReadAndWrite);
        let item = GlobalStatePathAccessItem::new("/d/a", access.value());
        list.add(item);

        let dec = new_dec("test");
        let item = GlobalStatePathAccessItem::new_group("/d/a", None, Some(dec.clone()), AccessPermissions::ReadOnly as u8);

        list.add(item);

        let device = DeviceId::default();
        let item = GlobalStatePathAccessItem::new_group("/d/a", Some(device.object_id().clone()), None, AccessPermissions::ReadOnly as u8);

        list.add(item);

        let s = serde_json::to_string(&list).unwrap();
        println!("{}", s);

        // same zone, same dec
        let source = RequestSourceInfo {
            zone: DeviceZoneInfo {
                device: None,
                zone: None,
                zone_category: DeviceZoneCategory::CurrentDevice,
            },
            dec: owner_dec.clone(),
        };

        let ret = GlobalStateAccessRequest {
            path: Cow::Owned("/d/a/c/".to_owned()),
            dec: Cow::Owned(owner_dec.clone()),
            source: Cow::Borrowed(&source),
            op_type: RequestOpType::Write,
        };

        list.check(ret).unwrap();

        // same zone, diff dec
        let source = RequestSourceInfo {
            zone: DeviceZoneInfo {
                device: None,
                zone: None,
                zone_category: DeviceZoneCategory::CurrentDevice,
            },
            dec: dec.clone(),
        };

        let ret = GlobalStateAccessRequest {
            path: Cow::Owned("/d/a/c/".to_owned()),
            dec: Cow::Owned(owner_dec.clone()),
            source: Cow::Borrowed(&source),
            op_type: RequestOpType::Read,
        };

        list.check(ret).unwrap();

        // same zone, diff dec, write
        let source = RequestSourceInfo {
            zone: DeviceZoneInfo {
                device: None,
                zone: None,
                zone_category: DeviceZoneCategory::CurrentDevice,
            },
            dec: dec.clone(),
        };

        let ret = GlobalStateAccessRequest {
            path: Cow::Owned("/d/a/c/".to_owned()),
            dec: Cow::Owned(owner_dec.clone()),
            source: Cow::Borrowed(&source),
            op_type: RequestOpType::Write,
        };

        list.check(ret).unwrap_err();

        // test remove
        let device = DeviceId::default();
        let item = GlobalStatePathAccessItem::new_group("/d/a", Some(device.object_id().clone()), None, 0);
        list.remove(item).unwrap();

        let device = DeviceId::default();
        let item = GlobalStatePathAccessItem::new_group("/a/b", Some(device.object_id().clone()), None, 0);
        let ret = list.remove(item);
        assert!(ret.is_none());

        let device = DeviceId::default();
        let item = GlobalStatePathAccessItem::new_group("/d/a/c", Some(device.object_id().clone()), None, 0);
        let ret = list.remove(item);
        assert!(ret.is_none());

    }
}
