use super::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

use async_std::sync::{Arc, Mutex};
use std::collections::{hash_map::Entry, HashMap};

#[derive(Clone)]
struct ObjectAccessCache {
    last_access_time: u64,
    last_access_rpath: Option<String>,

    cache_create_tick: u64,
}

// Reduce the write frequency of last_access updates to the underlying database
pub(super) struct NamedObjectMetaWithAccessCache {
    next: NamedObjectMetaRef,
    cache: Arc<Mutex<HashMap<ObjectId, ObjectAccessCache>>>,
}

impl NamedObjectMetaWithAccessCache {
    pub fn new(next: NamedObjectMetaRef) -> Self {
        Self {
            next,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn start(&self) {
        let cache = self.cache.clone();
        let next = self.next.clone();
        async_std::task::spawn(async move {
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(60)).await;
                Self::flush_cache(&next, &cache).await;
            }
        });
    }

    async fn flush_cache(
        next: &NamedObjectMetaRef,
        cache: &Arc<Mutex<HashMap<ObjectId, ObjectAccessCache>>>,
    ) {
        let mut list = Vec::with_capacity(128);

        {
            let mut cache = cache.lock().await;
            let now = bucky_time_now();
            cache.retain(|object_id, info| {
                if now - info.cache_create_tick >= 1000 * 1000 * 60 * 10 {
                    let req = NamedObjectMetaUpdateLastAccessRequest {
                        object_id: object_id.to_owned(),
                        last_access_time: info.last_access_time,
                        last_access_rpath: info.last_access_rpath.clone(),
                    };
                    list.push(req);
                    false
                } else {
                    true
                }
            });
        }

        for req in list {
            let _ = next.update_last_access(&req).await;
        }
    }
}

#[async_trait::async_trait]
impl NamedObjectMeta for NamedObjectMetaWithAccessCache {
    async fn put_object(
        &self,
        req: &NamedObjectMetaPutObjectRequest,
    ) -> BuckyResult<NamedObjectMetaPutObjectResponse> {
        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: &NamedObjectMetaGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectMetaData>> {
        let ret = self.next.get_object(req).await?;
        match ret {
            Some(mut item) => {
                let cache = self.cache.lock().await;
                if let Some(info) = cache.get(&req.object_id) {
                    item.last_access_rpath = info.last_access_rpath.clone();
                }
                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    async fn delete_object(
        &self,
        req: &NamedObjectMetaDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectMetaDeleteObjectResponse> {
        self.next.delete_object(req).await
    }

    async fn exists_object(&self, req: &NamedObjectMetaExistsObjectRequest) -> BuckyResult<bool> {
        self.next.exists_object(req).await
    }

    async fn update_last_access(
        &self,
        req: &NamedObjectMetaUpdateLastAccessRequest,
    ) -> BuckyResult<bool> {
        let mut cache = self.cache.lock().await;
        match cache.entry(req.object_id) {
            Entry::Occupied(mut o) => {
                let item = o.get_mut();
                if item.last_access_time >= req.last_access_time {
                    return Ok(false);
                }

                item.last_access_rpath = req.last_access_rpath.clone();
                item.last_access_time = req.last_access_time;
            }
            Entry::Vacant(v) => {
                let info = ObjectAccessCache {
                    last_access_time: req.last_access_time,
                    last_access_rpath: req.last_access_rpath.clone(),
                    cache_create_tick: bucky_time_now(),
                };
                v.insert(info);
            }
        }

        // FIXME always return true now, In some cases there may be a conflict: in case of object not exists
        Ok(true)
    }

    async fn update_object_meta(
        &self,
        req: &NamedObjectMetaUpdateObjectMetaRequest,
    ) -> BuckyResult<()> {
        self.next.update_object_meta(&req).await
    }

    async fn check_object_access(
        &self,
        req: &NamedObjectMetaCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        self.next.check_object_access(req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectMetaStat> {
        self.next.stat().await
    }

    fn bind_object_meta_access_provider(
        &self,
        object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    ) {
        self.next
            .bind_object_meta_access_provider(object_meta_access_provider)
    }
}
