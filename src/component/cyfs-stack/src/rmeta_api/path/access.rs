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

impl GlobalStatePathAccessList {
    pub fn new() -> Self {
        Self { list: vec![] }
    }

    // return true if any changed
    pub fn add(&mut self, mut item: GlobalStatePathAccessItem) -> bool {
        item.try_fix_path();

        if let Ok(i) = self.list.binary_search(&item) {
            if item == self.list[i] {
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

    pub fn remove(
        &mut self,
        mut item: GlobalStatePathAccessItem,
    ) -> Option<GlobalStatePathAccessItem> {
        item.try_fix_path();

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

    pub async fn check<'d, 'a, 'b>(
        &self,
        req: GlobalStateAccessRequest<'d, 'a, 'b>,
        current_device_id: &DeviceId,
        handler: &GlobalStatePathHandlerRef,
    ) -> BuckyResult<()> {
        let req_path = if req.path.ends_with('/') {
            req.path.clone()
        } else {
            Cow::Owned(format!("{}/", req.path))
        };

        for item in &self.list {
            if req_path.starts_with(item.path.as_str()) {
                match &item.access {
                    GlobalStatePathGroupAccess::Default(access) => {
                        let mask = req.source.mask(&req.dec, req.permissions);
                        if mask & access == mask {
                            info!("raccess match item: req={}, access={}", req, item);
                            return Ok(());
                        } else {
                            let msg = format!(
                                "raccess reject by item: device={}, req={}, access={}",
                                current_device_id, req, item
                            );
                            warn!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                        }
                    }
                    GlobalStatePathGroupAccess::Specified(user) => {
                        if user.compare(&req.source) {
                            let permissons = req.permissions as u8;
                            if permissons & user.access == permissons {
                                info!("raccess match item: req={}, access={}", req, item);
                                return Ok(());
                            } else {
                                let msg = format!(
                                    "raccess reject by item: device={}, req={}, access={}",
                                    current_device_id, req, item
                                );
                                warn!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                            }
                        } else {
                            // Find next path access item
                            continue;
                        }
                    }
                    GlobalStatePathGroupAccess::Handler => {
                        let handler_req = GlobalStatePathHandlerRequest {
                            dec_id: req.dec.as_ref().to_owned(),
                            source: req.source.as_ref().to_owned(),
                            req_path: req.path.as_ref().to_owned(),
                            req_query_string: req.query_string.as_ref().map(|v| v.to_string()),
                            permissions: req.permissions,
                        };

                        match handler.on_check(handler_req).await? {
                            true => {
                                return Ok(());
                            }
                            false => {
                                let msg = format!(
                                    "raccess reject by handler: device={}, req={}, access={}",
                                    current_device_id, req, item
                                );
                                warn!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                            }
                        }
                    }
                }
            }
        }

        let msg = format!(
            "raccess reject by default: device={}, req={}",
            current_device_id, req
        );
        warn!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
    }
}

#[cfg(test)]
mod test_path_access {
    use super::*;
    use cyfs_core::*;

    use std::sync::Arc;

    fn new_dec(name: &str) -> ObjectId {
        let owner_id = PeopleId::default();
        let dec_id = DecApp::generate_id(owner_id.into(), name);

        info!("generage random dec: name={}, dec_id={}", name, dec_id);

        dec_id
    }

    struct DummyHandler {}

    #[async_trait::async_trait]
    impl GlobalStatePathHandler for DummyHandler {
        async fn on_check(&self, req: GlobalStatePathHandlerRequest) -> BuckyResult<bool> {
            unreachable!();
        }
    }

    #[test]
    fn test() {
        async_std::task::block_on(test_run());
    }

    async fn test_run() {
        cyfs_base::init_simple_log("test_path_access", None);

        let handler = Arc::new(Box::new(DummyHandler {}) as Box<dyn GlobalStatePathHandler>);

        let current_device_id = DeviceId::default();
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
        let item = GlobalStatePathAccessItem::new_group(
            "/d/a",
            None,
            None,
            Some(dec.clone()),
            AccessPermissions::ReadOnly as u8,
        );

        list.add(item);

        let device = DeviceId::default();
        let item = GlobalStatePathAccessItem::new_group(
            "/d/a",
            Some(device.object_id().clone()),
            None,
            None,
            AccessPermissions::ReadOnly as u8,
        );

        list.add(item);

        let s = serde_json::to_string(&list).unwrap();
        println!("{}", s);

        // same zone, same dec
        let source = RequestSourceInfo {
            protocol: RequestProtocol::Native,
            zone: DeviceZoneInfo {
                device: None,
                zone: None,
                zone_category: DeviceZoneCategory::CurrentDevice,
            },
            dec: owner_dec.clone(),
            verified: None,
        };

        let ret = GlobalStateAccessRequest {
            path: Cow::Owned("/d/a/c/".to_owned()),
            query_string: None,
            dec: Cow::Owned(owner_dec.clone()),
            source: Cow::Borrowed(&source),
            permissions: AccessPermissions::WriteOnly,
        };

        list.check(ret, &current_device_id, &handler).await.unwrap();

        // same zone, diff dec
        let source = RequestSourceInfo {
            protocol: RequestProtocol::Native,
            zone: DeviceZoneInfo {
                device: None,
                zone: None,
                zone_category: DeviceZoneCategory::CurrentDevice,
            },
            dec: dec.clone(),
            verified: None,
        };

        let ret = GlobalStateAccessRequest {
            path: Cow::Owned("/d/a/c/".to_owned()),
            query_string: None,
            dec: Cow::Owned(owner_dec.clone()),
            source: Cow::Borrowed(&source),
            permissions: AccessPermissions::ReadOnly,
        };

        list.check(ret, &current_device_id, &handler).await.unwrap();

        // same zone, diff dec, write
        let source = RequestSourceInfo {
            protocol: RequestProtocol::Native,
            zone: DeviceZoneInfo {
                device: None,
                zone: None,
                zone_category: DeviceZoneCategory::CurrentDevice,
            },
            dec: dec.clone(),
            verified: None,
        };

        let ret = GlobalStateAccessRequest {
            path: Cow::Owned("/d/a/c/".to_owned()),
            query_string: None,
            dec: Cow::Owned(owner_dec.clone()),
            source: Cow::Borrowed(&source),
            permissions: AccessPermissions::WriteOnly,
        };

        list.check(ret, &current_device_id, &handler).await.unwrap_err();

        // test remove
        let device = DeviceId::default();
        let item = GlobalStatePathAccessItem::new_group(
            "/d/a",
            Some(device.object_id().clone()),
            None,
            None,
            0,
        );
        list.remove(item).unwrap();

        let device = DeviceId::default();
        let item = GlobalStatePathAccessItem::new_group(
            "/a/b",
            Some(device.object_id().clone()),
            None,
            None,
            0,
        );
        let ret = list.remove(item);
        assert!(ret.is_none());

        let device = DeviceId::default();
        let item = GlobalStatePathAccessItem::new_group(
            "/d/a/c",
            Some(device.object_id().clone()),
            None,
            None,
            0,
        );
        let ret = list.remove(item);
        assert!(ret.is_none());
    }
}
