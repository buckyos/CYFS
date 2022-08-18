use crate::prelude::*;
use crate::blob::*;
use crate::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

struct NamedObjectLocalStorage {
    meta: Box<dyn NamedObjectMeta>,
    blob: Box<dyn BlobStorage>,
}

impl NamedObjectLocalStorage {
    async fn put_object(
        &self,
        request: &NamedObjectCachePutObjectRequest,
    ) -> BuckyResult<NamedObjectCachePutObjectResponse> {
        let meta_req = self.gen_meta_put_request(request);
        let meta_ret = self.meta.put_object(&meta_req).await?;

        info!(
            "meta put object success! request={}, ret={}",
            meta_req, meta_ret
        );

        let put_ret;
        match meta_ret.result {
            NamedObjectMetaPutObjectResult::Accept => {
                self.blob.put_object(request.object.clone()).await?;
                put_ret = NamedObjectCachePutObjectResult::Accept;
            }
            NamedObjectMetaPutObjectResult::AlreadyExists => {
                let ret = self.blob.get_object(&request.object.object_id).await?;
                match ret {
                    Some(data) => {
                        let ret = self.merge_body_and_signs(data, &request.object)?;
                        if let Some(data) = ret {
                            self.blob.put_object(data).await?;
                            put_ret = NamedObjectCachePutObjectResult::Merged;
                        } else {
                            put_ret = NamedObjectCachePutObjectResult::AlreadyExists;
                        }
                    }
                    None => {
                        warn!(
                            "object not exists in blob storage, now will save! obj={}",
                            request.object.object_id
                        );
                        self.blob.put_object(request.object.clone()).await?;

                        put_ret = NamedObjectCachePutObjectResult::AlreadyExists;
                    }
                }
            }
            NamedObjectMetaPutObjectResult::Updated => {
                let ret = self.blob.get_object(&request.object.object_id).await?;
                match ret {
                    Some(data) => {
                        let ret = self.merge_body_and_signs(data, &request.object)?;
                        if let Some(data) = ret {
                            self.blob.put_object(data).await?;
                            put_ret = NamedObjectCachePutObjectResult::Merged;
                        } else {
                            put_ret = NamedObjectCachePutObjectResult::Updated;
                        }
                    }
                    None => {
                        warn!(
                            "object not exists in blob storage, now will save! obj={}",
                            request.object.object_id
                        );
                        self.blob.put_object(request.object.clone()).await?;

                        put_ret = NamedObjectCachePutObjectResult::Updated;
                    }
                }
            }
        }

        let resp = NamedObjectCachePutObjectResponse {
            result: put_ret,
            update_time: meta_ret.update_time,
            expires_time: meta_ret.expired_time,
        };

        Ok(resp)
    }

    fn gen_meta_put_request(
        &self,
        request: &NamedObjectCachePutObjectRequest,
    ) -> NamedObjectMetaPutObjectRequest {
        let obj = request.object.object.as_ref().unwrap();

        let update_time = obj.update_time();
        let expired_time = obj.expired_time();
        let owner_id = obj.owner().to_owned();

        NamedObjectMetaPutObjectRequest {
            source: request.source.clone(),
            object_id: request.object.object_id.clone(),
            owner_id,
            update_time,
            expired_time,

            storage_category: request.storage_category,
            context: request.context.clone(),
            last_access_rpath: request.last_access_rpath.clone(),
            access_string: 0,
        }
    }

    fn merge_body_and_signs(
        &self,
        current: NONObjectInfo,
        new: &NONObjectInfo,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        let mut current_obj: AnyNamedObject = current.object.unwrap().into();
        let new_obj = new.object.as_ref().unwrap();

        let mut changed = false;
        match current_obj.update_time() {
            None => {
                match new_obj.update_time() {
                    None => {
                        // both has no body, merge desc signs
                        let count = current_obj.signs_mut().unwrap().merge_ex(
                            &new_obj.signs().unwrap(),
                            true,
                            false,
                        );
                        if count > 0 {
                            changed = true;
                        }
                    }
                    Some(new_update_time) => {
                        info!(
                            "object had new body! now will replace body and signs! obj={}, new={}",
                            current.object_id, new_update_time
                        );
                        // new obj has body, now will set body & body signs, and merge desc signs
                        current_obj.set_body_expect(&new_obj);
                        current_obj.signs_mut().unwrap().merge_ex(
                            &new_obj.signs().unwrap(),
                            true,
                            true,
                        );
                        changed = true;
                    }
                }
            }
            Some(current_update_time) => {
                match new_obj.update_time() {
                    None => {
                        // new obj has not body, we should keep the current body, merge desc signs
                        let count = current_obj.signs_mut().unwrap().merge_ex(
                            &new_obj.signs().unwrap(),
                            true,
                            false,
                        );
                        if count > 0 {
                            changed = true;
                        }
                    }
                    Some(new_update_time) => {
                        // compare the body update_time, keep the newer one
                        if current_update_time < new_update_time {
                            info!("object body update time is newer! now will replace body and signs! obj={}, current={}, new={}", current.object_id, current_update_time, new_update_time);
                            current_obj.set_body_expect(&new_obj);
                            current_obj.signs_mut().unwrap().clear_body_signs();
                            current_obj.signs_mut().unwrap().merge_ex(
                                &new_obj.signs().unwrap(),
                                true,
                                true,
                            );

                            changed = true;
                        } else if current_update_time == new_update_time {
                            // FIXME should we check the body hash?
                            let count = current_obj.signs_mut().unwrap().merge_ex(
                                &new_obj.signs().unwrap(),
                                true,
                                true,
                            );
                            if count > 0 {
                                info!("object body update time is the same! now will merge desc signs! obj={}, update_time={}", current.object_id, current_update_time);
                                changed = true;
                            }
                        } else {
                            info!("object body update time is older! now will keep body and merge desc signs! obj={}, current={}, new={}", current.object_id, current_update_time, new_update_time);
                            let count = current_obj.signs_mut().unwrap().merge_ex(
                                &new_obj.signs().unwrap(),
                                true,
                                false,
                            );
                            if count > 0 {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        if !changed {
            Ok(None)
        } else {
            let object_raw = current_obj.to_vec()?;
            let info =
                NONObjectInfo::new(current.object_id, object_raw, Some(Arc::new(current_obj)));
            Ok(Some(info))
        }
    }

    async fn get_object(
        &self,
        req: &NamedObjectCacheGetObjectRequest1,
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>> {
        let meta_req = NamedObjectMetaGetObjectRequest {
            source: req.source.clone(),
            object_id: req.object_id.clone(),
            last_access_rpath: req.last_access_rpath.clone(),
        };

        let meta_ret = self.meta.get_object(&meta_req).await?;
        if meta_ret.is_none() {
            // FIXME verify the blod not exists?
            return Ok(None);
        }

        let mut meta = meta_ret.unwrap();

        // try get object data from blob
        let blob_ret = self.blob.get_object(&meta.object_id).await?;

        if let Some(data) = &blob_ret {
            // meta and blob maybe unmatch?
            let obj = data.object.as_ref().unwrap();
            let update_time = obj.update_time();

            if meta.update_time != update_time {
                warn!(
                    "object meta and blob update_time not match! obj={}, meta={:?}, blob={:?}",
                    req.object_id, meta.update_time, update_time
                );
                meta.update_time = update_time;
            }
        } else {
            warn!("object blob missing! obj={}", req.object_id);
        }

        let resp = NamedObjectCacheObjectData {
            object: blob_ret,
            meta,
        };

        Ok(Some(resp))
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest1,
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>> {
        let meta_req = NamedObjectMetaDeleteObjectRequest {
            source: req.source.clone(),
            object_id: req.object_id.clone(),
        };

        let meta_ret = self.meta.delete_object(&meta_req).await?;
        let resp = match meta_ret {
            Some(meta) => {
                // then remove object data from blob
                let object = match self.blob.delete_object(&meta_req.object_id).await {
                    Ok(Some(data)) => Some(data),
                    Ok(None) => {
                        warn!(
                            "delete object but remove from blob not found! obj={}",
                            req.object_id
                        );
                        None
                    }
                    Err(e) => {
                        error!(
                            "delete object but remove from blob failed! obj={}, {}",
                            req.object_id, e
                        );
                        None
                    }
                };

                Some(NamedObjectCacheObjectData { meta, object })
            }
            None => {
                // still try remove object data from blob
                match self.blob.delete_object(&meta_req.object_id).await {
                    Ok(Some(_data)) => {
                        warn!(
                            "delete object not exists in meta but exsits in blob! obj={}",
                            meta_req.object_id
                        );
                    }
                    Ok(None) => {}
                    Err(e) => {
                        error!(
                            "try delete object from blob failed! obj={}, {}",
                            req.object_id, e
                        );
                    }
                };

                None
            }
        };

        Ok(resp)
    }

    async fn exists_object(
        &self,
        req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse> {
        let meta_req = NamedObjectMetaExistsObjectRequest {
            source: req.source.clone(),
            object_id: req.object_id.clone(),
        };

        let meta = self.meta.exists_object(&meta_req).await?;

        let object = match self.blob.exists_object(&req.object_id).await {
            Ok(ret) => ret,
            Err(e) => {
                warn!(
                    "exists object from blob but error! obj={}, {}",
                    req.object_id, e
                );
                false
            }
        };

        if meta != object {
            warn!(
                "exists object but meta and blob not match! obj={}, meta={}, blob={}",
                req.object_id, meta, object
            );

            if !meta && object {
                // FIXME delete object from blob?;
            }
        }

        let resp = NamedObjectCacheExistsObjectResponse { meta, object };

        Ok(resp)
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat1> {
        let meta = self.meta.stat().await?;
        let blob = self.blob.stat().await?;

        let resp = NamedObjectCacheStat1 {
            count: meta.count,
            storage_size: meta.storage_size + blob.storage_size,
        };

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl NamedObjectCache1 for NamedObjectLocalStorage {
    async fn put_object(
        &self,
        req: &NamedObjectCachePutObjectRequest,
    ) -> BuckyResult<NamedObjectCachePutObjectResponse> {
        Self::put_object(&self, req).await
    }

    async fn get_object(
        &self,
        req: &NamedObjectCacheGetObjectRequest1,
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>> {
        Self::get_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest1,
    ) -> BuckyResult<Option<NamedObjectCacheObjectData>> {
        Self::delete_object(&self, req).await
    }

    async fn exists_object(
        &self,
        req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse> {
        Self::exists_object(&self, req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat1> {
        Self::stat(&self).await
    }
}
