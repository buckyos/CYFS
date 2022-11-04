use crate::root_state_api::ObjectMapNOCCacheAdapter;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NONObjectMapLoader {
    noc: NamedObjectCacheRef,
    root_cache: ObjectMapRootCacheRef,
    op_env_cache: ObjectMapOpEnvCacheRef,
}

impl NONObjectMapLoader {
    pub fn new(noc: NamedObjectCacheRef) -> Self {
        let noc_cache = ObjectMapNOCCacheAdapter::new_noc_cache(noc.clone());
        let root_cache = ObjectMapRootMemoryCache::new_ref(noc_cache, 60 * 5, 1024);
        let op_env_cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        Self {
            noc,
            root_cache,
            op_env_cache,
        }
    }

    pub async fn load(&self, req: NONGetObjectInputRequest) -> BuckyResult<NONObjectInfo> {
        let inner_path = req.inner_path.unwrap();

        info!(
            "will get objectmap with inner path: {}, {}",
            req.object_id, inner_path
        );

        assert_eq!(req.object_id.obj_type_code(), ObjectTypeCode::ObjectMap);

        let path = ObjectMapPath::new(req.object_id.clone(), self.op_env_cache.clone());
        let ret = path.get_by_path(&inner_path).await?;
        if ret.is_none() {
            let msg = format!(
                "get_by_path but inner path not found! object={}, inner_path={}",
                req.object_id, inner_path,
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let object_id = ret.unwrap();

        let ret = if object_id.obj_type_code() == ObjectTypeCode::ObjectMap {
            let obj = self.root_cache.get_object_map(&object_id).await?;
            match obj {
                Some(obj) => {
                    let object = {
                        let object_map = obj.lock().await;
                        object_map.clone()
                    };

                    let object_raw = object.to_vec()?;
                    let object = AnyNamedObject::Standard(StandardObject::ObjectMap(object));

                    Some((Arc::new(object), object_raw))
                }
                None => None,
            }
        } else {
            self.load_object_from_noc(req.common.source, &object_id)
                .await?
        };

        if ret.is_none() {
            let msg = format!(
                "get_by_path but object not found! object={}, inner_path={}",
                req.object_id, inner_path,
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (object, object_raw) = ret.unwrap();

        let info = NONObjectInfo {
            object_id: object_id.to_owned(),
            object_raw,
            object: Some(object),
        };

        Ok(info)
    }

    async fn load_object_from_noc(
        &self,
        source: RequestSourceInfo,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<(Arc<AnyNamedObject>, Vec<u8>)>> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            object_id: object_id.clone(),
            source,
            last_access_rpath: None,
        };

        let resp = self.noc.get_object(&noc_req).await.map_err(|e| {
            error!("load object from noc error! id={}, {}", object_id, e);
            e
        })?;

        match resp {
            Some(resp) => Ok(Some((resp.object.object.unwrap(), resp.object.object_raw))),
            None => Ok(None),
        }
    }
}
