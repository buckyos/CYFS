use super::meta_cache::*;
use cyfs_base::{
    AnyNamedObject, BuckyError, BuckyErrorCode, BuckyResult, DeviceId, ObjectId, ObjectTypeCode,
    RawDecode, RawEncode,
};

use cyfs_lib::*;
use cyfs_noc::*;

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(RawEncode, RawDecode)]
struct SyncState {
    last_check_time: u64,
}

struct MetaSync {
    noc: NamedObjectCacheRef,
    meta_cache: NamedObjectCacheRef,
    state: NOCRawStorage<HashMap<ObjectId, SyncState>>,
}

impl MetaSync {
    pub fn new(noc: NamedObjectCacheRef, meta_cache: NamedObjectCacheRef) -> Self {
        let state = NOCRawStorage::new("", noc.clone());
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
