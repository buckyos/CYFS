use super::meta_cache::*;
use cyfs_base::{
    AnyNamedObject, BuckyError, BuckyErrorCode, BuckyResult, DeviceId, ObjectId, ObjectTypeCode,
    RawDecode, RawEncode,
};

use cyfs_noc::*;
use cyfs_lib::*;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(RawEncode, RawDecode)]
struct SyncState {
    last_check_time: u64,
}

struct MetaSync {
    noc: Box<dyn NamedObjectCache>,
    meta_cache: Box<dyn NamedObjectCache>,
    state: NOCRawStorage<HashMap<ObjectId, SyncState>>,
}

impl MetaSync {
    pub fn new(noc: Box<dyn NamedObjectCache>, meta_cache: Box<dyn NamedObjectCache>) -> Self {
        let state = NOCRawStorage::new("", noc.clone_noc());
        Self {
            noc,
            meta_cache,
            state,
        }
    }

    fn select(&self) {
        let mut filter = NamedObjectCacheSelectObjectFilter::default();
        filter.obj_type_code = Some(ObjectTypeCode::Device);
        //self.noc.select_object(filter: &NamedObjectCacheSelectObjectFilter, opt: Option<NamedObjectCacheSelectObjectOption>)
    }
}
