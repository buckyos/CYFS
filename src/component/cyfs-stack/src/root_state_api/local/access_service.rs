use super::super::core::*;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalStateAccessService {
    device_id: DeviceId,
    root_state: Arc<GlobalStateManager>,
    noc: Arc<Box<dyn NamedObjectCache>>,
}

impl GlobalStateAccessService {
    pub fn new(
        device_id: DeviceId,
        root_state: Arc<GlobalStateManager>,
        noc: Box<dyn NamedObjectCache>,
    ) -> Self {
        Self {
            device_id,
            root_state,
            noc: Arc::new(noc),
        }
    }

    pub fn clone_processor(&self) -> GlobalStateAccessInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    async fn get_object_id(
        &self,
        dec_id: &Option<ObjectId>,
        inner_path: &str,
    ) -> BuckyResult<(ObjectId, ObjectMapRootCacheRef, (ObjectId, u64))> {
        match dec_id {
            None => {
                let (root, revision) = self.root_state.get_current_root();
                let root_cache = self.root_state.root_cache().clone();
                Ok((root.clone(), root_cache, (root, revision)))
            }
            Some(dec_id) => {
                let dec_root_manager = self.root_state.get_dec_root_manager(dec_id, false).await?;
                let op_env = dec_root_manager.create_op_env().await?;
                let ret = op_env.get_by_path(inner_path).await?;
                if ret.is_none() {
                    let msg = format!(
                        "get_by_path but not found! dec={}, path={}",
                        dec_id, inner_path,
                    );
                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }

                let dec_root = op_env.root().clone();
                let (root, revision) = self.root_state.get_dec_relation_root_info(&dec_root);

                let object_id = ret.unwrap();
                let root_cache = dec_root_manager.root_cache().clone();
                Ok((object_id, root_cache, (root, revision)))
            }
        }
    }

    pub async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathInputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathInputResponse> {
        info!("on access get_object_by_path request: {}", req);

        let resp = self
            .get_by_path_impl(&req.common.dec_id, &req.inner_path)
            .await?;

        Ok(resp)
    }

    // for http protocol's get method
    async fn get_by_path_impl(
        &self,
        dec_id: &Option<ObjectId>,
        inner_path: &str,
    ) -> BuckyResult<RootStateAccessGetObjectByPathInputResponse> {
        let (object_id, root_cache, root_info) = self.get_object_id(dec_id, inner_path).await?;

        let object_resp = match object_id.obj_type_code() {
            ObjectTypeCode::Chunk => NONGetObjectInputResponse::new(object_id, vec![], None),
            _ => {
                let ret = if object_id.obj_type_code() == ObjectTypeCode::ObjectMap {
                    let obj = root_cache.get_object_map(&object_id).await?;
                    match obj {
                        Some(obj) => {
                            let object = {
                                let object_map = obj.lock().await;
                                object_map.clone()
                            };

                            let object_raw = object.to_vec()?;
                            let object =
                                AnyNamedObject::Standard(StandardObject::ObjectMap(object));

                            Some((Arc::new(object), object_raw))
                        }
                        None => None,
                    }
                } else {
                    self.load_object_from_noc(&object_id).await?
                };

                if ret.is_none() {
                    let msg = format!(
                        "get_by_path but object not found! dec={:?}, path={}, object={}",
                        dec_id, inner_path, object_id
                    );
                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }

                let (object, object_raw) = ret.unwrap();

                let mut resp = NONGetObjectInputResponse::new(object_id, object_raw, Some(object));
                resp.init_times()?;
                resp
            }
        };

        let resp = RootStateAccessGetObjectByPathInputResponse {
            object: object_resp,
            root: root_info.0,
            revision: root_info.1,
        };

        Ok(resp)
    }

    async fn load_object_from_noc(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<(Arc<AnyNamedObject>, Vec<u8>)>> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            protocol: NONProtocol::Native,
            object_id: object_id.clone(),
            source: self.device_id.clone(),
        };

        let resp = self.noc.get_object(&noc_req).await.map_err(|e| {
            error!("load object from noc error! id={}, {}", object_id, e);
            e
        })?;

        match resp {
            Some(resp) => {
                assert!(resp.object.is_some());
                assert!(resp.object_raw.is_some());

                Ok(Some((resp.object.unwrap(), resp.object_raw.unwrap())))
            }
            None => Ok(None),
        }
    }

    async fn list(
        &self,
        req: RootStateAccessListInputRequest,
    ) -> BuckyResult<RootStateAccessListInputResponse> {
        info!("on access list request: {}", req);

        let (target, root_cache, root_info) = self
            .get_object_id(&req.common.dec_id, &req.inner_path)
            .await?;

        if target.obj_type_code() != ObjectTypeCode::ObjectMap {
            let msg =
                format!(
                "list but target object is not objectmap! dec={:?}, path={}, target={}, type={:?}",
                req.common.dec_id, req.inner_path, target, target.obj_type_code(),
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
        }

        let obj = root_cache.get_object_map(&target).await?;
        if obj.is_none() {
            let msg = format!(
                "list but target object not found! dec={:?}, path={}, target={}",
                req.common.dec_id, req.inner_path, target,
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }
        let obj = obj.unwrap();

        let page_index = req.page_index.unwrap_or(0) as usize;
        let page_size = req.page_size.unwrap_or(1024) as usize;
        if page_size == 0 {
            return Ok(RootStateAccessListInputResponse {
                list: vec![],
                root: root_info.0,
                revision: root_info.1,
            });
        }

        let begin = page_size * page_index;
        let obj = obj.lock().await;

        // TODO it maybe cached during next incoming list request with inc page_index
        let op_env_cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache);
        let mut it = if begin > 0 {
            let mut it = ObjectMapIterator::new(true, &obj, op_env_cache);
            let count = it.skip(&obj, begin).await?;
            if count < begin {
                return Ok(RootStateAccessListInputResponse {
                    list: vec![],
                    root: root_info.0,
                    revision: root_info.1,
                });
            }

            it.into_iterator()
        } else {
            ObjectMapIterator::new(false, &obj, op_env_cache)
        };

        let list = it.next(&obj, page_size).await?;

        Ok(RootStateAccessListInputResponse {
            list: list.list,
            root: root_info.0,
            revision: root_info.1,
        })
    }
}

#[async_trait::async_trait]
impl GlobalStateAccessInputProcessor for GlobalStateAccessService {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathInputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathInputResponse> {
        Self::get_object_by_path(self, req).await
    }

    async fn list(
        &self,
        req: RootStateAccessListInputRequest,
    ) -> BuckyResult<RootStateAccessListInputResponse> {
        Self::list(self, req).await
    }
}
