use super::cache::{MetaMemoryCacheForObject, MetaMemoryCacheForName};
use super::fail_cache::*;
use super::meta_cache::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_meta_lib::{MetaClient, MetaClientHelper, MetaMinerTarget};

use async_trait::async_trait;
use std::sync::Arc;


// FIXME: Choose a more appropriate cache duration, theoretically it should be longer!
const OBJECT_CACHE_TIMEOUT_IN_SECS: u64 = 60 * 15;
const NAME_CACHE_TIMEOUT_IN_SECS: u64 = 60 * 15;

#[derive(Clone)]
pub(crate) struct RawMetaCache {
    noc: NamedObjectCacheRef,
    meta_client: Arc<MetaClient>,
    device_id: DeviceId,

    // Cache in memory
    object_memory_cache: MetaMemoryCacheForObject,
    name_memory_cache: MetaMemoryCacheForName,

    // Error cache, avoid quickly initiating query operations to the chain in short time
    fail_cache: MetaFailCache,
}

impl RawMetaCache {
    pub fn new(target: MetaMinerTarget, noc: NamedObjectCacheRef) -> MetaCacheRef {
        info!("raw meta cache: {}", target.to_string());
        let meta_client =
            MetaClient::new_target(target).with_timeout(std::time::Duration::from_secs(60 * 2));

        let ret = Self {
            noc,
            meta_client: Arc::new(meta_client),
            device_id: DeviceId::default(),

            
            object_memory_cache: MetaMemoryCacheForObject::new(OBJECT_CACHE_TIMEOUT_IN_SECS),
            name_memory_cache: MetaMemoryCacheForName::new(NAME_CACHE_TIMEOUT_IN_SECS),

            fail_cache: MetaFailCache::new(),
        };

        Arc::new(Box::new(ret))
    }

    async fn get_from_meta(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<MetaObjectCacheData>> {
        let key = MetaCacheKey::Object(object_id.to_owned());
        if let Some(e) = self.fail_cache.get(&key) {
            let ret = match e.code() {
                BuckyErrorCode::NotFound => Ok(None),
                _ => Err(e),
            };

            return ret;
        }

        MetaClientHelper::get_object(&self.meta_client, object_id)
            .await
            .map(|ret| {
                self.fail_cache.on_success();
                match ret {
                    Some((object, object_raw)) => {
                        let object = Arc::new(object);
                        let resp = MetaObjectCacheData { object, object_raw };
                        Some(resp)
                    }
                    None => {
                        self.fail_cache
                            .add(key.clone(), BuckyError::from(BuckyErrorCode::NotFound));
                        None
                    }
                }
            })
            .map_err(|e| {
                self.fail_cache.add(key, e.clone());
                e
            })
    }

    // 返回值表示对象有没有发生更新
    async fn update_noc(
        &self,
        object_id: &ObjectId,
        resp: &MetaObjectCacheData,
    ) -> BuckyResult<bool> {
        let object = NONObjectInfo::new(
            object_id.clone(),
            resp.object_raw.clone(),
            Some(resp.object.clone()),
        );

        let req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_system().protocol(RequestProtocol::Meta),
            storage_category: NamedObjectStorageCategory::Cache,
            context: None,
            last_access_rpath: None,
            object,
            access_string: None,
        };

        match self.noc.put_object(&req).await {
            Ok(resp) => match resp.result {
                NamedObjectCachePutObjectResult::AlreadyExists => {
                    debug!("meta object alreay in noc: {}", object_id);
                    Ok(false)
                }
                NamedObjectCachePutObjectResult::Merged => {
                    info!("meta object alreay in noc but signs merged: {}", object_id);
                    Ok(true)
                }
                NamedObjectCachePutObjectResult::Accept => {
                    info!("put meta object to noc success! {}", object_id);
                    Ok(true)
                }
                NamedObjectCachePutObjectResult::Updated => {
                    info!("put meta object to noc and updated! {}", object_id);
                    Ok(true)
                }
            },
            Err(e) => {
                error!("put_object to noc error! {}, {}", object_id, e);
                Err(e)
            }
        }
    }

    fn get_object_from_cache(&self, object_id: &ObjectId) -> Option<MetaObjectCacheData> {
        match self.object_memory_cache.get(object_id) {
            Some(object_raw) => match AnyNamedObject::raw_decode(&object_raw) {
                Ok((object, _)) => {
                    let object = Arc::new(object);
                    let resp = MetaObjectCacheData { object, object_raw };
                    Some(resp)
                }
                Err(e) => {
                    error!("invalid cached object format! obj={} err={}", object_id, e);
                    None
                }
            },
            None => None,
        }
    }

    pub async fn get_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<MetaObjectCacheData>> {
        // First lookup in memory
        if let Some(ret) = self.get_object_from_cache(object_id) {
            return Ok(Some(ret));
        }

        // Then try get from meta chain via network
        let resp = self.get_from_meta(object_id).await?;

        // Cache the result if success
        if let Some(data) = &resp {
            // cache in memory for later use
            self.object_memory_cache.add(object_id.to_owned(), data.object_raw.clone());

            // save to noc
            let _r = self.update_noc(object_id, data).await;
        }

        Ok(resp)
    }

    // true 更新了noc
    // false 对象没有发生改变
    pub async fn flush_object(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        self.fail_cache
            .remove(&MetaCacheKey::Object(object_id.to_owned()));
        let resp = self.get_from_meta(object_id).await?;

        match resp {
            Some(data) => self.update_noc(object_id, &data).await,
            None => Ok(false),
        }
    }


    async fn get_name(&self, name: &str) -> BuckyResult<Option<(NameInfo, NameState)>> {
        if let Some(ret) = self.name_memory_cache.get(name) {
            return Ok(ret);
        }

        match self.get_name_impl(name).await {
            Ok(v) => {
                if v.is_none() {
                    warn!("get name from meta chain but not found! name={}", name);
                }

                self.name_memory_cache.add(name.to_owned(), v.clone());
                Ok(v)
            }
            Err(e) => {
                if e.code() == BuckyErrorCode::NotFound {
                    warn!(
                        "get name from meta chain but not found! name={} err={}",
                        name, e
                    );

                    Ok(None)
                } else {
                    let msg = format!("get name from meta chain failed! name={} err={}", name, e);
                    error!("{}", msg);

                    Err(BuckyError::new(e.code(), msg))
                }
            }
        }
    }

    async fn get_name_impl(&self, name: &str) -> BuckyResult<Option<(NameInfo, NameState)>> {
        let key = MetaCacheKey::Name(name.to_owned());
        if let Some(e) = self.fail_cache.get(&key) {
            return Err(e);
        }

        self.meta_client
            .get_name(name)
            .await
            .map_err(|e| {
                self.fail_cache.add(key, e.clone());
                e
            })
            .map(|v| {
                self.fail_cache.on_success();
                v
            })
    }
}

#[async_trait]
impl MetaCache for RawMetaCache {
    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<MetaObjectCacheData>> {
        RawMetaCache::get_object(&self, object_id).await
    }

    async fn flush_object(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        RawMetaCache::flush_object(&self, object_id).await
    }

    async fn get_name(&self, name: &str) -> BuckyResult<Option<(NameInfo, NameState)>> {
        RawMetaCache::get_name(&self, name).await
    }

    fn clone_meta(&self) -> Box<dyn MetaCache> {
        Box::new(self.clone()) as Box<dyn MetaCache>
    }
}
