use super::client::*;
use cyfs_base::*;

use async_std::sync::Mutex as AsyncMutex;
use std::sync::Arc;

pub struct MetaClientHelper;

impl MetaClientHelper {
    pub async fn get_object(
        meta_client: &MetaClient,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<(AnyNamedObject, Vec<u8>)>> {
        let object_raw = match meta_client.get_raw_data(object_id).await {
            Ok(v) => v,
            Err(e) => {
                if e.code() == BuckyErrorCode::NotFound {
                    warn!(
                        "get object from meta chain but not found! obj={} err={}",
                        object_id, e
                    );

                    return Ok(None);
                } else {
                    let msg = format!(
                        "load object from meta chain failed! obj={} err={}",
                        object_id, e
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(e.code(), msg));
                }
            }
        };

        info!("get object from meta success: {}", object_id);
        let (object, _) = AnyNamedObject::raw_decode(&object_raw).map_err(|e| {
            let msg = format!("invalid object format! obj={} err={}", object_id, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        // 校验一下对象id，看是否匹配
        let id = object.calculate_id();
        if id != *object_id {
            let msg = format!(
                "get object from meta but got unmatch object id! expected={}, got={}",
                object_id, id
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        let resp = (object, object_raw);

        Ok(Some(resp))
    }
}

struct MetaClientObjectItem {
    object_raw: Option<Vec<u8>>,
}

impl MetaClientObjectItem {
    pub fn into_object_raw(&self) -> Option<Vec<u8>> {
        self.object_raw
            .as_ref()
            .map(|object_raw| object_raw.clone())
    }
}

#[derive(Clone)]
pub struct MetaClientHelperWithObjectCache {
    objects: Arc<AsyncMutex<lru_time_cache::LruCache<ObjectId, MetaClientObjectItem>>>,
}

impl MetaClientHelperWithObjectCache {
    pub fn new(timeout: std::time::Duration, capacity: usize) -> Self {
        Self {
            objects: Arc::new(AsyncMutex::new(
                lru_time_cache::LruCache::with_expiry_duration_and_capacity(timeout, capacity),
            )),
        }
    }

    pub async fn get_object(
        &self,
        meta_client: &MetaClient,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<(AnyNamedObject, Vec<u8>)>> {
        Ok(self
            .get_object_raw(meta_client, object_id)
            .await?
            .map(|object_raw| {
                let (object, _) = AnyNamedObject::raw_decode(&object_raw).unwrap();
                (object, object_raw)
            }))
    }

    pub async fn get_object_raw(
        &self,
        meta_client: &MetaClient,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<Vec<u8>>> {
        let mut list = self.objects.lock().await;
        if let Some(item) = list.peek(object_id) {
            return Ok(item.into_object_raw());
        }

        let ret = MetaClientHelper::get_object(meta_client, object_id).await?;
        let ret = ret.map(|(_, object_raw)| object_raw);
        let item = MetaClientObjectItem {
            object_raw: ret.as_ref().map(|object_raw| object_raw.clone()),
        };

        list.insert(object_id.to_owned(), item);

        Ok(ret)
    }
}
