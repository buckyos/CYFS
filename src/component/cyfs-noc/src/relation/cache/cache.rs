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
    pub fn new(
        next: NamedObjectRelationCacheRef,
        timeout_in_secs: u64,
        capacity: usize,
        missing_capacity: usize,
    ) -> Self {
        let cache = lru_time_cache::LruCache::with_expiry_duration_and_capacity(
            std::time::Duration::from_secs(timeout_in_secs),
            capacity,
        );

        let missing_cache = lru_time_cache::LruCache::with_expiry_duration_and_capacity(
            std::time::Duration::from_secs(timeout_in_secs),
            missing_capacity,
        );

        Self {
            next,
            cache: AsyncMutex::new(cache),
            missing_cache: Mutex::new(missing_cache),
        }
    }

    pub fn is_missing(&self, req: &NamedObjectRelationCacheGetRequest) -> bool {
        let cache = self.missing_cache.lock().unwrap();
        cache.peek(&req.cache_key).is_some()
    }

    pub async fn get(
        &self,
        req: &NamedObjectRelationCacheGetRequest,
    ) -> Option<NamedObjectRelationCacheData> {
        {
            let mut cache = self.cache.lock().await;
            let ret = cache.get(&req.cache_key);
            ret.cloned()
        }
    }

    pub async fn cache(
        &self,
        req: &NamedObjectRelationCacheGetRequest,
        data: &NamedObjectRelationCacheData,
    ) {
        // Concurrency is allowed here
        assert!(data.target_object_id.is_some());

        let mut cache = self.cache.lock().await;
        let _ret = cache.insert(req.cache_key.clone(), data.clone());
        // assert!(ret.is_none());
    }

    // ensure starts with / and end with no /
    fn fix_inner_path(origin_path: &str) -> Option<String> {
        let path = origin_path.trim();
        if path == "/" {
            if path == origin_path {
                return None;
            } else {
                return Some(path.to_owned());
            }
        }

        // At the end / need to be removed
        let path = path.trim_end_matches("/");
        if path.starts_with("/") {
            if path == origin_path {
                None
            } else {
                Some(path.to_owned())
            }
        } else {
            Some(format!("/{}", path))
        }
    }
}

use std::borrow::Cow;

#[async_trait::async_trait]
impl NamedObjectRelationCache for NamedObjectRelationCacheMemoryCache {
    async fn put(&self, req: &NamedObjectRelationCachePutRequest) -> BuckyResult<()> {
        let req = if req.cache_key.relation_type == NamedObjectRelationType::InnerPath {
            match Self::fix_inner_path(&req.cache_key.relation) {
                Some(inner_path) => {
                    let mut req = req.clone();
                    req.cache_key.relation = inner_path;
                    Cow::Owned(req)
                }
                None => {
                    Cow::Borrowed(req)
                }
            }
        } else {
            Cow::Borrowed(req)
        };

        if req.target_object_id.is_some() {
            let ret = self.next.put(&req).await;

            {
                let mut cache = self.cache.lock().await;
                cache.remove(&req.cache_key);
            }

            {
                let mut cache = self.missing_cache.lock().unwrap();
                cache.remove(&req.cache_key);
            }

            ret
        } else {
            let mut cache = self.missing_cache.lock().unwrap();
            let _ret = cache.insert(req.cache_key.clone(), ());

            Ok(())
        }
    }

    async fn get(
        &self,
        req: &NamedObjectRelationCacheGetRequest,
    ) -> BuckyResult<Option<NamedObjectRelationCacheData>> {
        let req = if req.cache_key.relation_type == NamedObjectRelationType::InnerPath {
            match Self::fix_inner_path(&req.cache_key.relation) {
                Some(inner_path) => {
                    let mut req = req.clone();
                    req.cache_key.relation = inner_path;
                    Cow::Owned(req)
                }
                None => {
                    Cow::Borrowed(req)
                }
            }
        } else {
            Cow::Borrowed(req)
        };

        let cache_item = self.get(&req).await;
        if cache_item.is_some() {
            return Ok(cache_item);
        }

        if self.is_missing(&req) {
            return Ok(Some(NamedObjectRelationCacheData {
                target_object_id: None,
            }));
        }

        let ret = self.next.get(&req).await?;

        if ret.is_some() {
            self.cache(&req, ret.as_ref().unwrap()).await;
        }

        Ok(ret)
    }
}
