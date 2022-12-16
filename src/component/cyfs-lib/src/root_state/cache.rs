use cyfs_base::*;
use crate::*;

use std::sync::Arc;

pub struct ObjectMapNOCCacheAdapter {
    noc: NamedObjectCacheRef,
}

impl ObjectMapNOCCacheAdapter {
    pub fn new(noc: NamedObjectCacheRef) -> Self {
        Self {
            noc,
        }
    }

    pub fn new_noc_cache(
        noc: NamedObjectCacheRef,
    ) -> ObjectMapNOCCacheRef {
        let ret = Self::new(noc);
        Arc::new(Box::new(ret) as Box<dyn ObjectMapNOCCache>)
    }
}

#[async_trait::async_trait]
impl ObjectMapNOCCache for ObjectMapNOCCacheAdapter {
    async fn exists(&self, dec_id: Option<ObjectId>, object_id: &ObjectId) -> BuckyResult<bool> {
        let noc_req = NamedObjectCacheExistsObjectRequest {
            object_id: object_id.clone(),
            source: RequestSourceInfo::new_local_dec_or_system(dec_id),
        };

        let resp = self.noc.exists_object(&noc_req).await.map_err(|e| {
            error!("exists object map from noc error! id={}, {}", object_id, e);
            e
        })?;

        if resp.meta && resp.object {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get_object_map(&self, dec_id: Option<ObjectId>, object_id: &ObjectId) -> BuckyResult<Option<ObjectMap>> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            source: RequestSourceInfo::new_local_dec_or_system(dec_id),
            object_id: object_id.clone(),
            last_access_rpath: None,
        };

        let resp = self.noc.get_object(&noc_req).await.map_err(|e| {
            error!("load object map from noc error! id={}, {}", object_id, e);
            e
        })?;

        match resp {
            Some(resp) => {
                match ObjectMap::raw_decode(&resp.object.object_raw) {
                    Ok((obj, _)) => {
                        // 首次加载后，直接设置id缓存，减少一次id计算
                        obj.direct_set_object_id_on_init(object_id);

                        Ok(Some(obj))
                    }
                    Err(e) => {
                        error!("decode ObjectMap object error: id={}, {}", object_id, e);
                        Err(e)
                    }
                }
            }
            None => Ok(None),
        }
    }

    async fn put_object_map(&self, dec_id: Option<ObjectId>, object_id: ObjectId, object: ObjectMap) -> BuckyResult<()> {

        let object_raw = object.to_vec().unwrap();
        let object = AnyNamedObject::Standard(StandardObject::ObjectMap(object));
        let object = NONObjectInfo::new(object_id, object_raw, Some(Arc::new(object)));
 
        let req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_dec_or_system(dec_id),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: Some(AccessString::dec_default().value()),
        };

        self.noc.put_object(&req).await.map_err(|e| {
            error!(
                "insert object map to noc error! id={}, dec={:?}, {}",
                object_id, dec_id, e
            );
            e
        })?;

        Ok(())
    }
}
