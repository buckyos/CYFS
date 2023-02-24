use cyfs_base::*;
use cyfs_debug::Mutex;

use lru_time_cache::{Entry, LruCache};
use std::sync::Arc;

// 最小的重试间隔
const RETRY_INTERVAL_MIN: u64 = 1000 * 1000 * 5;

// 最大的重试间隔
const RETRY_INTERVAL_MAX: u64 = 1000 * 1000 * 60 * 5;

#[derive(Hash, Clone, PartialOrd, Eq, PartialEq, Ord)]
pub(crate) enum MetaCacheKey {
    Object(ObjectId),
    Name(String),
}

#[derive(Clone)]
struct MetaCacheItem {
    error: BuckyError,
}

struct MetaFailCacheImpl {
    list: LruCache<MetaCacheKey, MetaCacheItem>,

    // 网络错误后，避免过多请求，需要增加重试间隔
    retry_interval: u64,

    last_failed_tick: u64,
    last_failed_error: Option<BuckyError>,
}

// NotFound错误，缓存固定5分钟
// 其余错误，可能是访问Meta出错了，按照指数增加重试次数

impl MetaFailCacheImpl {
    pub fn new() -> Self {
        let list = LruCache::with_expiry_duration(std::time::Duration::from_secs(60 * 5));

        Self {
            list,
            retry_interval: RETRY_INTERVAL_MIN,
            last_failed_tick: 0,
            last_failed_error: None,
        }
    }

    // 错误分两种，meta返回的正常错误，和异常错误，异常错误下需要采用规避错误
    fn is_meta_error(error: &BuckyError) -> bool {
        match error.code() {
            BuckyErrorCode::NotFound | BuckyErrorCode::CodeError | BuckyErrorCode::MetaError(_) => {
                true
            }
            _ => false,
        }
    }

    pub fn on_success(&mut self) {
        self.retry_interval = RETRY_INTERVAL_MIN;
        self.last_failed_tick = 0;
        self.last_failed_error = None;
    }

    fn on_fail(&mut self, error: BuckyError) {
        self.retry_interval *= 2;
        if self.retry_interval > RETRY_INTERVAL_MAX {
            self.retry_interval = RETRY_INTERVAL_MAX;
        }

        self.last_failed_tick = bucky_time_now();
        self.last_failed_error = Some(error);
    }

    pub fn add(&mut self, key: MetaCacheKey, error: BuckyError) {
        if Self::is_meta_error(&error) {
            self.on_success();
            self.cache_item(key, error);
        } else {
            self.on_fail(error);
        }
    }

    fn cache_item(&mut self, key: MetaCacheKey, error: BuckyError) {
        let item = MetaCacheItem { error };
        match self.list.entry(key) {
            Entry::Occupied(o) => *o.into_mut() = item,
            Entry::Vacant(v) => {
                v.insert(item);
            }
        }
    }

    pub fn remove(&mut self, key: &MetaCacheKey) -> Option<BuckyError> {
        self.list.remove(&key).map(|v| v.error)
    }

    pub fn get(&mut self, key: &MetaCacheKey) -> Option<BuckyError> {
        // 首先查询是不是存在缓存
        // force remove expired items 
        self.list.iter();

        // use peek instead of get, do not update the last use timestamp!
        let ret = self.list.peek(&key).map(|v| v.error.clone());
        if ret.is_some() {
            return ret;
        }

        // 判断是不是在出错后的重试间隔内，避免短时间内发起大量重试
        if self.last_failed_tick > 0 {
            let now = bucky_time_now();
            if now - self.last_failed_tick < self.retry_interval {
                let err = self.last_failed_error.as_ref().unwrap().clone();
                warn!(
                    "get from meta still in error cache state, interval={}, {}",
                    self.retry_interval, err
                );
                return Some(err);
            }
        }

        None
    }
}

#[derive(Clone)]
pub(crate) struct MetaFailCache(Arc<Mutex<MetaFailCacheImpl>>);

impl MetaFailCache {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(MetaFailCacheImpl::new())))
    }

    pub fn on_success(&self) {
        self.0.lock().unwrap().on_success()
    }

    pub fn add(&self, key: MetaCacheKey, error: BuckyError) {
        self.0.lock().unwrap().add(key, error)
    }

    pub fn remove(&self, key: &MetaCacheKey) -> Option<BuckyError> {
        self.0.lock().unwrap().remove(key)
    }

    pub fn get(&self, key: &MetaCacheKey) -> Option<BuckyError> {
        self.0.lock().unwrap().get(key)
    }
}

#[cfg(test)]
mod fail_cache_tests {
    use super::*;

    #[async_std::test]
    async fn test() {
        cyfs_util::init_log("test-cache", Some("trace"));

        let cache = MetaFailCache::new();
        let cache1 = cache.clone();
        async_std::task::spawn(async move {
            loop {
                cache1.on_success();
                async_std::task::sleep(std::time::Duration::from_secs(60)).await;
            }
        });
        async_std::task::spawn(async move {
            let mut last_real_get = std::time::Instant::now();
            loop {
                let key = MetaCacheKey::Object(ObjectId::default());
                let v = cache.get(&key);
                // info!("get object: {:?}", v);
                if v.is_none() {
                    info!(
                        "will real get object, during={}s",
                        last_real_get.elapsed().as_secs()
                    );
                    last_real_get = std::time::Instant::now();
                    cache.add(key, BuckyError::from(BuckyErrorCode::Failed));
                } else {
                }
                async_std::task::sleep(std::time::Duration::from_secs(1)).await;
            }
        });

        async_std::task::sleep(std::time::Duration::from_secs(60 * 5)).await;
    }
}
