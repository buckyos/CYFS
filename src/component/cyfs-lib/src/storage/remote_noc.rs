use crate::non::*;
use crate::prelude::*;
use crate::NONOutputProcessorRef;
use cyfs_base::*;

use async_trait::async_trait;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct RemoteNamedObjectCache {
    non_service: NONOutputProcessorRef,
    device_id: DeviceId,
}

impl RemoteNamedObjectCache {
    pub fn new(non_service: NONOutputProcessorRef, device_id: &DeviceId) -> Self {
        Self {
            non_service,
            device_id: device_id.to_owned(),
        }
    }

    pub fn into_noc(self) -> NamedObjectCacheRef {
        Arc::new(Box::new(self))
    }
}

#[async_trait]
impl NamedObjectCache for RemoteNamedObjectCache {
    async fn put_object(
        &self,
        req: &NamedObjectCachePutObjectRequest,
    ) -> BuckyResult<NamedObjectCachePutObjectResponse> {
        let req = NONPutObjectOutputRequest::new_noc(
            req.object.object_id.clone(),
            req.object.object_raw.clone(),
        );

        let resp = self.non_service.put_object(req).await?;

        Ok(NamedObjectCachePutObjectResponse {
            result: resp.result.into(),
            expires_time: resp.object_expires_time,
            update_time: resp.object_update_time,
        })
    }

    async fn get_object_raw(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>> {
        let req = NONGetObjectOutputRequest::new_noc(req.object_id.clone(), None);

        match self.non_service.get_object(req).await {
            Ok(resp) => {
                // FIXME update the get_object resp to adapt the new noc get_object
                let data = NamedObjectCacheObjectRawData {
                    meta: {
                        let object = resp.object.object();

                        NamedObjectMetaData {
                            object_id: resp.object.object_id.clone(),
                            object_type: object.obj_type(),
                            owner_id: object.owner().to_owned(),
                            create_dec_id: cyfs_core::get_system_dec_app().to_owned(),
                            insert_time: bucky_time_now(),
                            update_time: bucky_time_now(),
                            object_create_time: match object.create_time() {
                                0 => None,
                                v @ _ => Some(v),
                            },
                            object_update_time: resp.object_update_time,
                            object_expired_time: resp.object_expires_time,
                            author: object.author().to_owned(),
                            dec_id: object.dec_id().to_owned(),
                            storage_category: NamedObjectStorageCategory::Storage,
                            context: None,
                            last_access_rpath: None,
                            access_string: 9,
                        }
                    },
                    object: Some(resp.object),
                };

                Ok(Some(data))
            }
            Err(e) => {
                if e.code() == BuckyErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse> {
        let noc_req = NONDeleteObjectOutputRequest::new_noc(req.object_id.clone(), None);

        match self.non_service.delete_object(noc_req).await {
            Ok(resp) => {
                let meta = if let Some(ref object) = resp.object {
                    let meta = NamedObjectMetaData {
                        object_id: object.object_id.clone(),
                        object_type: object.object().obj_type(),
                        owner_id: object.object().owner().to_owned(),
                        create_dec_id: cyfs_core::get_system_dec_app().to_owned(),
                        insert_time: bucky_time_now(),
                        update_time: bucky_time_now(),
                        object_create_time: match object.object().create_time() {
                            0 => None,
                            v @ _ => Some(v),
                        },
                        object_update_time: object.object().update_time(),
                        object_expired_time: object.object().expired_time(),
                        author: object.object().author().to_owned(),
                        dec_id: object.object().dec_id().to_owned(),
                        storage_category: NamedObjectStorageCategory::Storage,
                        context: None,
                        last_access_rpath: None,
                        access_string: 9,
                    };

                    Some(meta)
                } else {
                    None
                };

                let ret = NamedObjectCacheDeleteObjectResponse {
                    deleted_count: 1,
                    object: resp.object,
                    meta,
                };

                Ok(ret)
            }
            Err(e) => Err(e),
        }
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        unimplemented!();
    }

    async fn update_object_meta(
        &self,
        _req: &NamedObjectCacheUpdateObjectMetaRequest,
    ) -> BuckyResult<()> {
        unimplemented!();
    }

    async fn exists_object(
        &self,
        _req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse> {
        unimplemented!();
    }

    async fn check_object_access(
        &self,
        _req: &NamedObjectCacheCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        unimplemented!();
    }

    fn bind_object_meta_access_provider(
        &self,
        _object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    ) {
        unimplemented!();
    }
}
