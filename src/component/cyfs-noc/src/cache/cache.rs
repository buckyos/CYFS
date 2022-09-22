use crate::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

use lru_time_cache::LruCache;
use std::collections::HashSet;
use std::sync::Mutex;

type NamedObjectCacheItem = NamedObjectCacheObjectRawData;

pub struct NamedObjectCacheMemoryCache {
    meta: NamedObjectMetaRef,
    next: NamedObjectCacheRef,
    cache: Mutex<LruCache<ObjectId, NamedObjectCacheItem>>,
    missing_cache: Mutex<HashSet<ObjectId>>,
}

impl NamedObjectCacheMemoryCache {
    pub fn new(
        meta: NamedObjectMetaRef,
        next: NamedObjectCacheRef,
        timeout_in_secs: u64,
        capacity: usize,
    ) -> Self {
        let cache = lru_time_cache::LruCache::with_expiry_duration_and_capacity(
            std::time::Duration::from_secs(timeout_in_secs),
            capacity,
        );

        Self {
            meta,
            next,
            cache: Mutex::new(cache),
            missing_cache: Mutex::new(HashSet::new()),
        }
    }

    pub fn is_missing(&self, req: &NamedObjectCacheGetObjectRequest) -> bool {
        let cache = self.missing_cache.lock().unwrap();
        cache.contains(&req.object_id)
    }

    fn check_access(
        object_id: &ObjectId,
        access_string: u32,
        source: &RequestSourceInfo,
        create_dec_id: &ObjectId,
        op_type: RequestOpType,
    ) -> BuckyResult<()> {
        debug!(
            "noc cache will check access: object={}, access={}, source={}, create_dec={}",
            object_id,
            AccessString::new(access_string),
            source,
            create_dec_id
        );

        // system dec in current zone is always allowed
        if source.is_current_zone() {
            if source.is_system_dec() {
                return Ok(());
            }
        }

        // Check permission first
        let mask = source.mask(create_dec_id, op_type);

        if access_string & mask != mask {
            let msg = format!(
                "noc cache object access been rejected! obj={}, access={}, require access={}",
                object_id,
                AccessString::new(access_string),
                AccessString::new(mask)
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        Ok(())
    }

    pub fn get(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>> {
        let mut cache = self.cache.lock().unwrap();
        let ret = cache.get_mut(&req.object_id);
        if ret.is_none() {
            return Ok(None);
        }

        let item = ret.unwrap();

        // first check the access permissions
        if !req.source.is_verified() {
            Self::check_access(
                &req.object_id,
                item.meta.access_string,
                &req.source,
                &item.meta.create_dec_id,
                RequestOpType::Read,
            )?;
        }

        if item.meta.last_access_rpath != req.last_access_rpath {
            item.meta.last_access_rpath = req.last_access_rpath.to_owned();
        }

        Ok(Some(item.to_owned()))
    }

    pub fn cache(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
        data: &Option<NamedObjectCacheObjectRawData>,
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
impl NamedObjectCache for NamedObjectCacheMemoryCache {
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

    async fn get_object_raw(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>> {
        let cache_item = self.get(req)?;
        if cache_item.is_some() {
            // Update the last access info
            let update_req = NamedObjectMetaUpdateLastAccessRequest {
                object_id: req.object_id.clone(),
                last_access_time: bucky_time_now(),
                last_access_rpath: req.last_access_rpath.clone(),
            };

            if let Err(e) = self.meta.update_last_access(&update_req).await {
                error!(
                    "noc got from cache but update last access to meta failed! obj={}, {}",
                    req.object_id, e
                );
            }

            return Ok(cache_item);
        }

        if self.is_missing(req) {
            return Ok(None);
        }

        let ret = self.next.get_object_raw(req).await?;

        self.cache(req, &ret);

        Ok(ret)
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse> {
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

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        self.next.stat().await
    }
}
