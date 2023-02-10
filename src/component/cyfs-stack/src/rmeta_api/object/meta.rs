use super::selector::*;
use cyfs_base::*;
use cyfs_lib::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct ObjectMeta {
    // Object dynamic selector
    pub selector: ObjectSelector,

    // Access value
    pub access: GlobalStatePathGroupAccess,

    // Object referer's depth, default is 1
    pub depth: Option<u8>,
}

impl ObjectMeta {
    pub fn new(item: GlobalStateObjectMetaItem) -> BuckyResult<Self> {
        let selector = ObjectSelector::new(item.selector)?;
        Ok(Self {
            selector,
            access: item.access,
            depth: item.depth,
        })
    }

    pub fn new_uninit(item: GlobalStateObjectMetaItem) -> Self {
        let selector = ObjectSelector::new_uninit(item.selector);
        Self {
            selector,
            access: item.access,
            depth: item.depth,
        }
    }

    pub fn check_valid(&self) -> bool {
        self.access.check_valid()
    }
}

impl std::fmt::Display for ObjectMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({}, {}, {:?})",
            self.selector.exp(),
            self.access,
            self.depth
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct GlobalStateObjectMetaList {
    list: Vec<ObjectMeta>,
}

impl Default for GlobalStateObjectMetaList {
    fn default() -> Self {
        Self { list: vec![] }
    }
}

impl GlobalStateObjectMetaList {
    pub fn new() -> Self {
        Self { list: vec![] }
    }

    // return true if any changed
    pub fn add(&mut self, item: ObjectMeta) -> bool {
        if let Ok(i) = self.list.binary_search(&item) {
            if item == self.list[i] {
                return false;
            }

            info!("replace object meta: {} -> {}", self.list[i], item);
            self.list[i] = item;
        } else {
            info!("new object meta: {}", item);
            self.list.push(item);
        }

        true
    }

    pub fn remove(&mut self, item: &ObjectMeta) -> Option<ObjectMeta> {
        if let Ok(i) = self.list.binary_search(&item) {
            let item = self.list.remove(i);
            info!("remove object meta: {}", item);
            Some(item)
        } else {
            info!("remove object meta but not found: {}", item);
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

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn check(
        &self,
        target_dec_id: &ObjectId,
        object_data: &dyn ObjectSelectorDataProvider,
        source: &RequestSourceInfo,
        permissions: AccessPermissions,
        current_device_id: &DeviceId,
    ) -> BuckyResult<Option<()>> {
        if self.list.is_empty() {
            return Ok(None);
        }

        for item in &self.list {
            let ret = item.selector.eval(object_data);
            if ret.is_err() {
                error!(
                    "eval object meta exp error! exp={}, object={}, {}",
                    item.selector.exp(),
                    object_data.object_id(),
                    ret.unwrap_err()
                );
                continue;
            }
            let ret = ret.unwrap();
            if !ret {
                continue;
            }

            debug!(
                "eval object meta matched! exp={}, object={}",
                item.selector.exp(),
                object_data.object_id(),
            );

            match &item.access {
                GlobalStatePathGroupAccess::Default(access) => {
                    let mask = source.mask(target_dec_id, permissions);
                    if mask & access == mask {
                        info!("object meta match item: req={}, access={}", object_data.object_id(), item);
                        return Ok(Some(()));
                    } else {
                        let msg =
                            format!("object meta reject by item: device={}, req={}, access={}", current_device_id, object_data.object_id(), item);
                        warn!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                    }
                }
                GlobalStatePathGroupAccess::Specified(user) => {
                    if user.compare(&source) {
                        let permissons = permissions as u8;
                        if permissons & user.access == permissons {
                            info!("object meta match item: req={}, access={}", object_data.object_id(), item);
                            return Ok(Some(()));
                        } else {
                            let msg =
                                format!("object meta reject by item: device={}, req={}, access={}", current_device_id, object_data.object_id(), item);
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

        Ok(None)
    }
}
