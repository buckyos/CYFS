use crate::{access::RequestOpType, meta::*, prelude::*};
use cyfs_base::*;
use cyfs_lib::NONObjectInfo;

use lru_time_cache::LruCache;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

type NamedObjectCacheItem = NamedObjectCacheObjectData;

pub struct ObjectMapRootMemoryCache {
    next: NamedObjectCacheRef,
    cache: Mutex<LruCache<ObjectId, NamedObjectCacheItem>>,
    missing_cache: Mutex<HashSet<ObjectId>>,
}

impl ObjectMapRootMemoryCache {
    pub fn new(next: NamedObjectCacheRef, timeout_in_secs: u64, capacity: usize) -> Self {
        let cache = lru_time_cache::LruCache::with_expiry_duration_and_capacity(
            std::time::Duration::from_secs(timeout_in_secs),
            capacity,
        );

        Self {
            next,
            cache: Mutex::new(cache),
            missing_cache: Mutex::new(HashSet::new()),
        }
    }

    pub fn is_missing(&self, req: &NamedObjectCacheGetObjectRequest1) -> bool {
        let cache = self.missing_cache.lock().unwrap();
        cache.contains(&req.object_id)
    }

    pub fn get(
        &self,
        req: &NamedObjectCacheGetObjectRequest1,
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>> {
        let mut cache = self.cache.lock().unwrap();
        let ret = cache.get_mut(&req.object_id);
        if ret.is_none() {
            return Ok(None);
        }

        let item = ret.unwrap();

        // first check the access permissions
        let mask = req
            .source
            .mask(&item.meta.create_dec_id, RequestOpType::Read);
        if item.meta.access_string & mask != mask {
            let msg = format!("get object from cache but access been rejected! obj={}, access={:#o}, req access={:#o}", req.object_id, item.meta.access_string, mask);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        if item.meta.last_access_rpath != req.last_access_rpath {
            item.meta.last_access_rpath = req.last_access_rpath.to_owned();
        }

        // TODO update the meta last_access_time & last_access_path
        
        Ok(Some(item.to_owned()))
    }

    pub fn cache(
        &self,
        req: &NamedObjectCacheGetObjectRequest1,
        data: &Option<NamedObjectCacheObjectData>,
    ) {
        match data {
            Some(data) => {
                let item = data.to_owned();

                let mut cache = self.cache.lock().unwrap();
                let ret = cache.insert(req.object_id.to_owned(), item);
                assert!(ret.is_none());
            }
            None => {
                let mut cache = self.missing_cache.lock().unwrap();
                let ret = cache.insert(req.object_id.to_owned());
                assert!(ret);
            }
        }
    }
}

#[async_trait::async_trait]
impl NamedObjectCache1 for ObjectMapRootMemoryCache {
    async fn put_object(
        &self,
        req: &NamedObjectCachePutObjectRequest,
    ) -> BuckyResult<NamedObjectCachePutObjectResponse> {
        let ret = self.next.put_object(req).await;

        {
            let mut cache = self.cache.lock().unwrap();
            cache.remove(&req.object.object_id);
        }

        {
            let mut cache = self.missing_cache.lock().unwrap();
            cache.remove(&req.object.object_id);
        }

        ret
    }

    async fn get_object(
        &self,
        req: &NamedObjectCacheGetObjectRequest1,
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>> {
        let cache_item = self.get(req)?;
        if cache_item.is_some() {
            return Ok(cache_item);
        }

        if self.is_missing(req) {
            return Ok(None);
        }

        let ret = self.next.get_object(req).await?;

        self.cache(req, &ret);

        Ok(ret)
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest1,
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>> {
        {
            let mut cache = self.cache.lock().unwrap();
            cache.remove(&req.object_id);
        }

        self.next.delete_object(req).await
    }

    async fn exists_object(
        &self,
        req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse> {
        self.next.exists_object(req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat1> {
        self.next.stat().await
    }
}
