use cyfs_base::*;
use cyfs_lib::*;

use cyfs_debug::Mutex;
use lru_time_cache::{Entry, LruCache};
use std::sync::Arc;

#[derive(Hash, Clone, PartialOrd, Eq, PartialEq, Ord)]
pub(super) struct BdtDataAclCacheKey {
    pub source: DeviceId,
    pub referer: Option<String>,
    pub action: NDNAction,
}

#[derive(Clone)]
struct BdtDataAclCacheItem {
    result: BuckyResult<()>,
}

#[derive(Clone)]
pub(super) struct BdtDataAclCache {
    list: Arc<Mutex<LruCache<BdtDataAclCacheKey, BdtDataAclCacheItem>>>,
}

impl BdtDataAclCache {
    pub fn new() -> Self {
        let list = LruCache::with_expiry_duration(std::time::Duration::from_secs(60 * 5));

        Self {
            list: Arc::new(Mutex::new(list)),
        }
    }

    pub fn add(&self, key: BdtDataAclCacheKey, result: BuckyResult<()>) {
        let item = BdtDataAclCacheItem { result };

        let mut list = self.list.lock().unwrap();
        match list.entry(key) {
            Entry::Occupied(o) => *o.into_mut() = item,
            Entry::Vacant(v) => {
                v.insert(item);
            }
        }
    }

    pub fn get(&self, key: &BdtDataAclCacheKey) -> Option<BuckyResult<()>> {
        let mut list = self.list.lock().unwrap();
        list.get(&key).map(|v| v.result.clone())
    }
}
