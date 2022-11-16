use crate::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

use async_std::sync::Mutex as AsyncMutex;
use cyfs_debug::Mutex;
use lru_time_cache::LruCache;
use std::collections::HashSet;
use std::sync::Arc;

type NamedObjectCacheItem = NamedObjectCacheObjectRawData;
type NamedObjectCacheItemRef = Arc<NamedObjectCacheItem>;

pub struct NamedObjectCacheMemoryCache {
    meta: NamedObjectMetaRef,
    next: NamedObjectCacheRef,
    cache: AsyncMutex<LruCache<ObjectId, NamedObjectCacheItemRef>>,
    missing_cache: Mutex<HashSet<ObjectId>>,

    access: NamedObjecAccessHelper,
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
            cache: AsyncMutex::new(cache),
            missing_cache: Mutex::new(HashSet::new()),
            access: NamedObjecAccessHelper::new(),
        }
    }

    pub fn is_missing(&self, req: &NamedObjectCacheGetObjectRequest) -> bool {
        let cache = self.missing_cache.lock().unwrap();
        cache.contains(&req.object_id)
    }

    pub async fn get(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>> {
        let item = {
            let mut cache = self.cache.lock().await;
            let ret = cache.get_mut(&req.object_id);
            if ret.is_none() {
                return Ok(None);
            }
            ret.unwrap().clone()
        };

        // first check the access permissions
        self.access
            .check_access_with_meta_data(
                &req.object_id,
                &req.source,
                &item.meta,
                &item.meta.create_dec_id,
                RequestOpType::Read,
            )
            .await?;

        if item.meta.last_access_rpath != req.last_access_rpath {
            todo!();
            // item.meta.last_access_rpath = req.last_access_rpath.to_owned();
        }

        Ok(Some((*item).to_owned()))
    }

    pub async fn cache(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
        data: &Option<NamedObjectCacheObjectRawData>,
    ) {
        match data {
            Some(data) => {
                let item = Arc::new(data.to_owned());

                let mut cache = self.cache.lock().await;
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

    async fn check_object_access(
        &self,
        req: &NamedObjectCacheCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        let item = {
            let mut cache = self.cache.lock().await;
            let ret = cache.get_mut(&req.object_id);
            if ret.is_none() {
                return Ok(None);
            }

            ret.unwrap().clone()
        };

        // check the access permissions
        self.access
            .check_access_with_meta_data(
                &req.object_id,
                &req.source,
                &item.meta,
                &item.meta.create_dec_id,
                req.required_access,
            )
            .await?;

        Ok(Some(()))
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
            let mut cache = self.cache.lock().await;
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
        let cache_item = self.get(req).await?;
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

        self.cache(req, &ret).await;

        Ok(ret)
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse> {
        {
            let mut cache = self.cache.lock().await;
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

    async fn update_object_meta(
        &self,
        req: &NamedObjectCacheUpdateObjectMetaRequest,
    ) -> BuckyResult<()> {
        self.next.update_object_meta(req).await?;

        {
            let mut cache = self.cache.lock().await;
            cache.remove(&req.object_id);
        }

        {
            let mut cache = self.missing_cache.lock().unwrap();
            cache.remove(&req.object_id);
        }

        Ok(())
    }

    async fn check_object_access(
        &self,
        req: &NamedObjectCacheCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        let ret = Self::check_object_access(&self, req).await?;
        if ret.is_some() {
            return Ok(ret);
        }

        self.next.check_object_access(req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        self.next.stat().await
    }

    fn bind_object_meta_access_provider(
        &self,
        object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    ) {
        self.access
            .bind_object_meta_access_provider(object_meta_access_provider.clone());
        self.next
            .bind_object_meta_access_provider(object_meta_access_provider);
    }
}
