use cyfs_base::*;
use cyfs_lib::*;

use async_std::sync::Mutex as AsyncMutex;
use cyfs_debug::Mutex;
use lru_time_cache::LruCache;

pub(crate) struct NamedObjectRelationCacheMemoryCache {
    next: NamedObjectRelationCacheRef,
    cache: AsyncMutex<LruCache<NamedObjectRelationCacheKey, NamedObjectRelationCacheData>>,
    missing_cache: Mutex<LruCache<NamedObjectRelationCacheKey, ()>>,
}

impl NamedObjectRelationCacheMemoryCache {
    pub fn new(next: NamedObjectRelationCacheRef, timeout_in_secs: u64, capacity: usize, missing_capacity: usize) -> Self {
        let cache = lru_time_cache::LruCache::with_expiry_duration_and_capacity(
            std::time::Duration::from_secs(timeout_in_secs),
            capacity,
        );

        let missing_cache = lru_time_cache::LruCache::with_capacity(
            missing_capacity,
        );

        Self {
            next,
            cache: AsyncMutex::new(cache),
            missing_cache: Mutex::new(missing_cache),
        }
    }

    pub fn is_missing(&self, req: &NamedObjectRelationCacheGetRequest) -> bool {
        let mut cache = self.missing_cache.lock().unwrap();
        cache.get(&req.cache_key).is_some()
    }

    pub async fn get(
        &self,
        req: &NamedObjectRelationCacheGetRequest,
    ) -> BuckyResult<Option<NamedObjectRelationCacheData>> {
        let item = {
            let mut cache = self.cache.lock().await;
            let ret = cache.get(&req.cache_key);
            if ret.is_none() {
                return Ok(None);
            }
            ret.unwrap().clone()
        };

        Ok(Some(item))
    }

    pub async fn cache(
        &self,
        req: &NamedObjectRelationCacheGetRequest,
        data: &Option<NamedObjectRelationCacheData>,
    ) {
        // Concurrency is allowed here
        match data {
            Some(data) => {
                let mut cache = self.cache.lock().await;
                let _ret = cache.insert(req.cache_key.clone(), data.clone());
                // assert!(ret.is_none());
            }
            None => {
                let mut cache = self.missing_cache.lock().unwrap();
                let _ret = cache.insert(req.cache_key.clone(), ());
                // assert!(ret.is_none());
            }
        }
    }
}

#[async_trait::async_trait]
impl NamedObjectRelationCache for NamedObjectRelationCacheMemoryCache {
    async fn put(&self, req: &NamedObjectRelationCachePutRequest) -> BuckyResult<()> {
        let ret = self.next.put(req).await;

        {
            let mut cache = self.cache.lock().await;
            cache.remove(&req.cache_key);
        }

        {
            let mut cache = self.missing_cache.lock().unwrap();
            cache.remove(&req.cache_key);
        }

        ret
    }

    async fn get(
        &self,
        req: &NamedObjectRelationCacheGetRequest,
    ) -> BuckyResult<Option<NamedObjectRelationCacheData>> {
        let cache_item = self.get(req).await?;
        if cache_item.is_some() {
            return Ok(cache_item);
        }

        if self.is_missing(req) {
            return Ok(None);
        }

        let ret = self.next.get(req).await?;

        self.cache(req, &ret).await;

        Ok(ret)
    }
}
