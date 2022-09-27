use super::fail_cache::*;
use super::meta_cache::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_meta_lib::{MetaClient, MetaClientHelper, MetaMinerTarget};

use async_trait::async_trait;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct RawMetaCache {
    noc: NamedObjectCacheRef,
    meta_client: Arc<MetaClient>,
    device_id: DeviceId,

    // 错误缓存，避免快速向链发起查询操作
    fail_cache: MetaFailCache,
}

impl RawMetaCache {
    pub fn new(target: MetaMinerTarget, noc: NamedObjectCacheRef) -> Self {
        info!("raw meta cache: {}", target.to_string());
        let meta_client = MetaClient::new_target(target);

        Self {
            noc,
            meta_client: Arc::new(meta_client),
            device_id: DeviceId::default(),
            fail_cache: MetaFailCache::new(),
        }
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
                    info!("meta object alreay in noc: {}", object_id);
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

    pub async fn get_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<MetaObjectCacheData>> {
        let resp = self.get_from_meta(object_id).await?;

        if let Some(data) = &resp {
            // 这里保存到noc
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
        match self.get_name_impl(name).await {
            Ok(v) => {
                if v.is_none() {
                    warn!("get name from meta chain but not found! name={}", name);
                }

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

                    Err(BuckyError::from(msg))
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
