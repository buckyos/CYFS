use crate::blob::*;
use crate::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::path::Path;
use std::sync::Arc;

pub struct NamedObjectLocalStorage {
    meta: NamedObjectMetaRef,
    blob: Box<dyn BlobStorage>,
}

impl NamedObjectLocalStorage {
    pub async fn new(isolate: &str) -> BuckyResult<Self> {
        let dir = cyfs_util::get_cyfs_root_path().join("data");
        let dir = if isolate.len() > 0 {
            dir.join(isolate)
        } else {
            dir
        };
        let dir = dir.join("named-object-cache");

        if !dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let msg = format!("create noc data dir error! dir={}, {}", dir.display(), e);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        // Init blob module
        let blob = Self::init_blob(&dir).await?;

        let meta = Self::init_meta(&dir)?;

        Ok(Self { blob, meta })
    }

    pub fn meta(&self) -> &NamedObjectMetaRef {
        &self.meta
    }

    async fn init_blob(root: &Path) -> BuckyResult<Box<dyn BlobStorage>> {
        let dir = root.join("objects");

        if !dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let msg = format!(
                    "create noc blob data dir error! dir={}, {}",
                    dir.display(),
                    e
                );
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        let blob = FileBlobStorage::new(dir);

        Ok(Box::new(blob))
    }

    fn init_meta(root: &Path) -> BuckyResult<NamedObjectMetaRef> {
        create_meta(root)
    }

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
                        if let Some((result, data)) = ret {
                            self.blob.put_object(data).await?;
                            put_ret = result;
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
                        if let Some((result, data)) = ret {
                            self.blob.put_object(data).await?;
                            put_ret = result;
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
            update_time: meta_ret.object_update_time,
            expires_time: meta_ret.object_expired_time,
        };

        Ok(resp)
    }

    fn gen_meta_put_request(
        &self,
        request: &NamedObjectCachePutObjectRequest,
    ) -> NamedObjectMetaPutObjectRequest {
        let obj = request.object.object.as_ref().unwrap();

        let object_create_time = match obj.create_time() {
            0 => None,
            v @ _ => Some(v)
        };

        let object_update_time = obj.update_time();
        let object_expired_time = obj.expired_time();
        let owner_id = obj.owner().to_owned();
        let dec_id = obj.dec_id().to_owned();
        let author = obj.author().to_owned();
        let prev = obj.prev().to_owned();
        let body_prev_version = obj.body_prev_version().to_owned();
        let ref_objs = obj.ref_objs().cloned();
        let nonce = obj.nonce().to_owned();

        // If the request does not specify an access string then try to use the default
        let access_string = match request.access_string {
            Some(v) => v,
            None => AccessString::default().value(),
        };

        NamedObjectMetaPutObjectRequest {
            source: request.source.clone(),
            object_id: request.object.object_id.clone(),
            owner_id,

            object_type: obj.obj_type(),
            insert_time: bucky_time_now(),
            object_create_time,
            object_update_time,
            object_expired_time,
            dec_id,
            author,
            prev,
            body_prev_version,
            ref_objs,
            nonce,

            storage_category: request.storage_category,
            context: request.context.clone(),
            last_access_rpath: request.last_access_rpath.clone(),
            access_string,
        }
    }

    fn merge_body_and_signs(
        &self,
        current: NONObjectInfo,
        new: &NONObjectInfo,
    ) -> BuckyResult<Option<(NamedObjectCachePutObjectResult, NONObjectInfo)>> {
        let mut current_obj: AnyNamedObject = current.object.unwrap().into();
        let new_obj = new.object.as_ref().unwrap();

        let mut changed = false;
        let mut result = NamedObjectCachePutObjectResult::AlreadyExists;
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
                            result = NamedObjectCachePutObjectResult::Merged;
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
                        result = NamedObjectCachePutObjectResult::Updated;
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
                            result = NamedObjectCachePutObjectResult::Merged;
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
                            result = NamedObjectCachePutObjectResult::Updated;
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
                                result = NamedObjectCachePutObjectResult::Merged;
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
                                result = NamedObjectCachePutObjectResult::Merged;
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
            Ok(Some((result, info)))
        }
    }

    async fn get_object_raw(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>> {
        let meta_req = NamedObjectMetaGetObjectRequest {
            source: req.source.clone(),
            object_id: req.object_id.clone(),
            last_access_rpath: req.last_access_rpath.clone(),
            flags: req.flags,
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
            let object_update_time = obj.update_time();

            if meta.object_update_time != object_update_time {
                warn!(
                    "object meta and blob update_time not match! obj={}, meta={:?}, blob={:?}",
                    req.object_id, meta.object_update_time, object_update_time
                );
                meta.object_update_time = object_update_time;
            }
        } else {
            warn!("object blob missing! obj={}", req.object_id);
        }

        let resp = NamedObjectCacheObjectRawData {
            object: blob_ret,
            meta,
        };

        Ok(Some(resp))
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse> {
        let meta_req = NamedObjectMetaDeleteObjectRequest {
            source: req.source.clone(),
            object_id: req.object_id.clone(),
            flags: req.flags,
        };

        let meta_resp = self.meta.delete_object(&meta_req).await?;
        let resp = match meta_resp.deleted_count {
            1 => {
                // then remove object data from blob
                let object = match self
                    .blob
                    .delete_object(&meta_req.object_id, req.flags)
                    .await
                {
                    Ok(resp) => match resp.delete_count {
                        1 => resp.object,
                        0 => {
                            warn!(
                                "delete object but remove from blob not found! obj={}",
                                req.object_id
                            );
                            None
                        }
                        _ => unreachable!(),
                    },
                    Err(e) => {
                        error!(
                            "delete object but remove from blob failed! obj={}, {}",
                            req.object_id, e
                        );
                        None
                    }
                };

                NamedObjectCacheDeleteObjectResponse {
                    deleted_count: meta_resp.deleted_count,
                    meta: meta_resp.object,
                    object,
                }
            }
            0 => {
                // still try remove object data from blob
                let object = match self
                    .blob
                    .delete_object(&meta_req.object_id, req.flags)
                    .await
                {
                    Ok(resp) => {
                        if resp.delete_count > 0 {
                            warn!(
                                "delete object not exists in meta but exsits in blob! obj={}",
                                meta_req.object_id
                            );
                        }

                        resp.object
                    }
                    Err(e) => {
                        error!(
                            "try delete object from blob failed! obj={}, {}",
                            req.object_id, e
                        );
                        None
                    }
                };

                NamedObjectCacheDeleteObjectResponse {
                    deleted_count: 0,
                    meta: None,
                    object,
                }
            }

            _ => unreachable!(),
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

    async fn update_object_meta(
        &self,
        req: &NamedObjectCacheUpdateObjectMetaRequest,
    ) -> BuckyResult<()> {
        self.meta.update_object_meta(req).await
    }

    async fn check_object_access(
        &self,
        req: &NamedObjectCacheCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        self.meta.check_object_access(req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        let meta = self.meta.stat().await?;
        let blob = self.blob.stat().await?;

        let resp = NamedObjectCacheStat {
            count: meta.count,
            storage_size: meta.storage_size + blob.storage_size,
        };

        Ok(resp)
    }

    async fn select_object(
        &self,
        req: &NamedObjectCacheSelectObjectRequest,
    ) -> BuckyResult<NamedObjectCacheSelectObjectResponse> {
        self.meta.select_object(req).await
    }
}

#[async_trait::async_trait]
impl NamedObjectCache for NamedObjectLocalStorage {
    async fn put_object(
        &self,
        req: &NamedObjectCachePutObjectRequest,
    ) -> BuckyResult<NamedObjectCachePutObjectResponse> {
        Self::put_object(&self, req).await
    }

    async fn get_object_raw(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>> {
        Self::get_object_raw(&self, req).await
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse> {
        Self::delete_object(&self, req).await
    }

    async fn exists_object(
        &self,
        req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse> {
        Self::exists_object(&self, req).await
    }

    async fn update_object_meta(
        &self,
        req: &NamedObjectCacheUpdateObjectMetaRequest,
    ) -> BuckyResult<()> {
        Self::update_object_meta(&self, req).await
    }

    async fn check_object_access(
        &self,
        req: &NamedObjectCacheCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        Self::check_object_access(&self, req).await
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        Self::stat(&self).await
    }

    async fn select_object(
        &self,
        req: &NamedObjectCacheSelectObjectRequest,
    ) -> BuckyResult<NamedObjectCacheSelectObjectResponse> {
        Self::select_object(self, req).await
    }

    fn bind_object_meta_access_provider(
        &self,
        object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    ) {
        self.meta.bind_object_meta_access_provider(object_meta_access_provider)
    }
}
