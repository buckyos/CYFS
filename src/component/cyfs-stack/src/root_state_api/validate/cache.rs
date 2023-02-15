use cyfs_base::*;

use lru_time_cache::LruCache;
use std::sync::Arc;
use cyfs_debug::Mutex;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct GlobalStatePathCacheKey {
    pub root: ObjectId,
    pub inner_path: String,
}

struct GlobalStatePathFailedItem {
    error: BuckyError,
}

struct GlobalStatePathCacheInner {
    success_cache: LruCache<GlobalStatePathCacheKey, ObjectId>,
    // TODO Add failed cache for some cases
    failed_cache: LruCache<GlobalStatePathCacheKey, GlobalStatePathFailedItem>,
}

impl GlobalStatePathCacheInner {
    pub fn new() -> Self {
        Self {
            success_cache: LruCache::with_capacity(1024 * 10),
            failed_cache: LruCache::with_expiry_duration_and_capacity(
                std::time::Duration::from_secs(60 * 60),
                1024,
            ),
        }
    }

    pub fn get(&mut self, key: &GlobalStatePathCacheKey) -> BuckyResult<Option<ObjectId>> {
        let ret = self.success_cache.get(key);
        if ret.is_some() {
            return Ok(ret.cloned());
        }

        // force remove expired items 
        self.failed_cache.iter();

        if let Some(item) = self.failed_cache.peek(key) {
            return Err(item.error.clone());
        }

        Ok(None)
    }

    pub fn on_success(&mut self, key: GlobalStatePathCacheKey, target: ObjectId) {
        if let Some(prev) = self.success_cache.insert(key, target) {
            warn!(
                "update global state validate success cache but already exists! prev={}",
                prev
            );
        }
    }

    pub fn on_failed(&mut self, key: GlobalStatePathCacheKey, error: BuckyError) {
        let item = GlobalStatePathFailedItem {
            error,
        };

        if let Some(prev) = self.failed_cache.insert(key, item) {
            warn!(
                "update global state validate failed cache but already exists! prev={}",
                prev.error
            );
        }
    }
}

#[derive(Clone)]
pub struct GlobalStatePathCache(Arc<Mutex<GlobalStatePathCacheInner>>);

impl GlobalStatePathCache {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(GlobalStatePathCacheInner::new())))
    }

    pub fn get(&self, key: &GlobalStatePathCacheKey) -> BuckyResult<Option<ObjectId>> {
        self.0.lock().unwrap().get(key)
    }

    pub fn on_success(&self, key: GlobalStatePathCacheKey, target: ObjectId) {
        self.0.lock().unwrap().on_success(key, target)
    }

    pub fn on_failed(&self, key: GlobalStatePathCacheKey, error: BuckyError) {
        self.0.lock().unwrap().on_failed(key, error)
    }
}