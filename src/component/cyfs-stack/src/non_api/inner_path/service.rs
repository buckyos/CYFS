use super::dir_loader::*;
use super::objectmap_loader::*;
use crate::non::*;
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// Used to support dir+innerpath and objectmap+innerpath modes
pub(crate) struct NONInnerPathServiceProcessor {
    next: NONInputProcessorRef,

    dir_loader: NONDirLoader,
    objectmap_loader: NONObjectMapLoader,

    noc: NamedObjectCacheRef,
    relation: NamedObjectRelationCacheRef,
}

impl NONInnerPathServiceProcessor {
    pub fn new(
        non_processor: NONInputProcessorRef,
        named_data_components: &NamedDataComponents,
        noc: NamedObjectCacheRef,
        relation: NamedObjectRelationCacheRef,
    ) -> NONInputProcessorRef {
        let dir_loader = NONDirLoader::new(
            non_processor.clone(),
            named_data_components.new_chunk_store_reader(),
        );

        // TODO objectmap loader should use non instead noc?
        let objectmap_loader = NONObjectMapLoader::new(noc.clone());

        let ret = Self {
            next: non_processor,
            dir_loader,
            objectmap_loader,
            noc,
            relation,
        };

        Arc::new(Box::new(ret))
    }

    async fn get_objectmap(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let ret = self.objectmap_loader.load(req).await?;

        let mut resp = NONGetObjectInputResponse::new_with_object(ret);
        resp.init_times()?;
        Ok(resp)
    }

    async fn get_dir(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let ret = self.dir_loader.get_dir(req).await?;
        let resp = match ret {
            DirResult::File((file, attr)) => {
                let object_raw = file.to_vec()?;
                let object = AnyNamedObject::Standard(StandardObject::File(file));

                let mut resp = NONGetObjectInputResponse::new(
                    object.object_id(),
                    object_raw,
                    Some(Arc::new(object)),
                );
                resp.attr = Some(attr);
                resp.init_times()?;
                resp
            }
            DirResult::Dir((dir, attr)) => {
                let object_raw = dir.to_vec()?;
                let object = AnyNamedObject::Standard(StandardObject::Dir(dir));

                let mut resp = NONGetObjectInputResponse::new(
                    object.object_id(),
                    object_raw,
                    Some(Arc::new(object)),
                );
                resp.attr = Some(attr);
                resp.init_times()?;
                resp
            }
        };

        Ok(resp)
    }

    async fn load_object_from_noc(
        &self,
        req: NONGetObjectInputRequest,
        object_id: &ObjectId,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            object_id: object_id.clone(),
            source: req.common.source,
            last_access_rpath: None,
            flags: 0,
        };

        let resp = self.noc.get_object(&noc_req).await.map_err(|e| {
            error!("load object from noc error! id={}, {}", object_id, e);
            e
        })?;

        match resp {
            Some(resp) => {
                let mut resp = NONGetObjectInputResponse::new_with_object(resp.object);
                resp.init_times()?;
                Ok(resp)
            }
            None => {
                let msg = format!(
                    "get object with inner_path but target object not found! object={}, inner_path={:?}, target={}",
                    req.object_id, req.inner_path, object_id,
                );
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONInnerPathServiceProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        if req.is_with_inner_path_relation() {
            // First try load relation from cache
            let cache_key = NamedObjectRelationCacheKey {
                object_id: req.object_id.clone(),
                relation_type: NamedObjectRelationType::InnerPath,
                relation: req.inner_path.as_ref().unwrap().clone(),
            };

            let relation_req = NamedObjectRelationCacheGetRequest {
                cache_key,
                flags: 0,
            };

            if let Ok(Some(data)) = self.relation.get(&relation_req).await {
                match data.target_object_id {
                    Some(id) => {
                        return self.load_object_from_noc(req, &id).await;
                    }
                    None => {
                        let msg = format!("get object with inner path but not found with missing cache! object={}, inner_path={}",
                        req.object_id, req.inner_path.as_ref().unwrap());
                        warn!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                    }
                }
            }

            // load indeed
            let ret = match req.object_id.obj_type_code() {
                ObjectTypeCode::ObjectMap => self.get_objectmap(req).await,
                ObjectTypeCode::Dir => self.get_dir(req).await,
                _ => unreachable!(),
            };

            match &ret {
                Ok(resp) => {
                    // cache relation
                    let req = NamedObjectRelationCachePutRequest {
                        cache_key: relation_req.cache_key,
                        target_object_id: Some(resp.object.object_id.clone()),
                    };

                    let _ = self.relation.put(&req).await;
                }
                Err(e) => {
                    if e.code() == BuckyErrorCode::NotFound
                        || e.code() == BuckyErrorCode::InnerPathNotFound
                    {
                        // cache relation
                        let req = NamedObjectRelationCachePutRequest {
                            cache_key: relation_req.cache_key,
                            target_object_id: None,
                        };

                        let _ = self.relation.put(&req).await;
                    }
                }
            }

            return ret;
        }

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        self.next.delete_object(req).await
    }
}
