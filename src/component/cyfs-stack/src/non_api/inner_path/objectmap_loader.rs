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

        // loader's dec already been system dec, will check access for the root object which will been required!
        let root_cache = ObjectMapRootMemoryCache::new_ref(None, noc_cache, 60 * 5, 1024);
        let op_env_cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        Self {
            noc,
            root_cache,
            op_env_cache,
        }
    }

    pub async fn load(&self, req: NONGetObjectInputRequest) -> BuckyResult<NONObjectInfo> {
        let inner_path = req.inner_path.unwrap();

        assert_eq!(req.object_id.obj_type_code(), ObjectTypeCode::ObjectMap);

        // first check access at object level
        let check_access_req = NamedObjectCacheCheckObjectAccessRequest {
            source: req.common.source.clone(),
            object_id: req.object_id.clone(),
            required_access: AccessPermissions::ReadOnly,
        };

        let ret = self.noc.check_object_access(&check_access_req).await?;
        if ret.is_none() {
            let msg = format!(
                "get_object with objectmap and inner_path but not found! object={}, inner_path={}",
                req.object_id, inner_path,
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        // load target object with inner_path
        let path = ObjectMapPath::new(req.object_id.clone(), self.op_env_cache.clone(), false);
        let ret = path.get_by_path(&inner_path).await?;
        if ret.is_none() {
            let msg = format!(
                "get object from objectmap with inner_path but target path not exists! objectmap={}, inner_path={}",
                req.object_id, inner_path,
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InnerPathNotFound, msg));
        }

        let object_id = ret.unwrap();

        let ret = if object_id.obj_type_code() == ObjectTypeCode::ObjectMap {
            let ret = self.root_cache.get_object_map(&object_id).await?;
            match ret {
                Some(object) => {
                    let object = {
                        let object_map = object.lock().await;
                        object_map.clone()
                    };

                    let object_raw = object.to_vec()?;
                    let object = AnyNamedObject::Standard(StandardObject::ObjectMap(object));

                    Some((Arc::new(object), object_raw))
                }
                None => None,
            }
        } else {
            self.load_object_from_noc(&object_id)
                .await?
        };

        if ret.is_none() {
            let msg = format!(
                "get object from objectmap with inner_path but target object not found! objectmap={}, inner_path={}, target={}",
                req.object_id, inner_path, object_id,
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (object, object_raw) = ret.unwrap();

        info!(
            "get object from objectmap with inner path: {}, {}, got={}",
            req.object_id, inner_path, object_id,
        );

        let info = NONObjectInfo {
            object_id: object_id.to_owned(),
            object_raw,
            object: Some(object),
        };

        Ok(info)
    }

    async fn load_object_from_noc(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<(Arc<AnyNamedObject>, Vec<u8>)>> {
        // objectmap + inner_path mode, only check the root object's access_string!
        let noc_req = NamedObjectCacheGetObjectRequest {
            object_id: object_id.clone(),
            source: RequestSourceInfo::new_local_system(),
            last_access_rpath: None,
            flags: 0,
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
