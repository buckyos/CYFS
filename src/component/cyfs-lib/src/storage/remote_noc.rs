use crate::base::*;
use crate::non::*;
use crate::NONOutputProcessorRef;
use cyfs_base::{BuckyErrorCode, BuckyResult, DeviceId};

use async_trait::async_trait;

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
}

#[async_trait]
impl NamedObjectCache for RemoteNamedObjectCache {
    async fn insert_object(
        &self,
        obj_info: &NamedObjectCacheInsertObjectRequest,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        let req = NONPutObjectOutputRequest::new_noc(
            obj_info.object_id.clone(),
            obj_info.object_raw.clone(),
        );

        let resp = self.non_service.put_object(req).await?;

        Ok(NamedObjectCacheInsertResponse {
            result: resp.result.into(),
            object_expires_time: resp.object_expires_time,
            object_update_time: resp.object_update_time,
        })
    }

    async fn get_object(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<ObjectCacheData>> {
        let req = NONGetObjectOutputRequest::new_noc(req.object_id.clone(), None);

        match self.non_service.get_object(req).await {
            Ok(resp) => {
                let mut data = ObjectCacheData {
                    protocol: NONProtocol::HttpLocal,
                    source: self.device_id.clone(),
                    object_id: resp.object.object_id.clone(),
                    dec_id: None,
                    object_raw: Some(resp.object.object_raw),
                    object: resp.object.object,
                    flags: 0u32,
                    create_time: 0u64,
                    update_time: 0u64,
                    insert_time: 0u64,
                    rank: OBJECT_RANK_NONE,
                };

                data.rebuild_object()?;

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

    async fn select_object(
        &self,
        req: &NamedObjectCacheSelectObjectRequest,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        let noc_filter = req.filter.clone().into();
        let noc_opt: Option<SelectOption> = match &req.opt {
            Some(opt) => Some(opt.clone().into()),
            None => None,
        };

        let noc_select_req = NONSelectObjectOutputRequest::new_noc(noc_filter, noc_opt);

        let resp = self.non_service.select_object(noc_select_req).await?;

        let mut list = Vec::new();
        for item in resp.objects {
            let object = item.object.unwrap();
            let mut data = ObjectCacheData {
                protocol: NONProtocol::HttpLocal,
                source: self.device_id.clone(),
                object_id: object.object_id,
                dec_id: None,
                object_raw: Some(object.object_raw),
                object: object.object,
                flags: 0u32,
                create_time: 0u64,
                update_time: 0u64,
                insert_time: 0u64,
                rank: OBJECT_RANK_NONE,
            };

            if let Err(e) = data.rebuild_object() {
                error!(
                    "rebuild object cache data error! obj={}, err={}",
                    data.object_id, e
                );
                continue;
            }

            list.push(data);
        }

        Ok(list)
    }

    async fn delete_object(
        &self,
        req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult> {
        let noc_req = NONDeleteObjectOutputRequest::new_noc(req.object_id.clone(), None);

        match self.non_service.delete_object(noc_req).await {
            Ok(resp) => {
                let object = if let Some(object) = resp.object {
                    let mut data = ObjectCacheData {
                        protocol: NONProtocol::HttpLocal,
                        source: self.device_id.clone(),
                        object_id: req.object_id.clone(),
                        dec_id: None,
                        object_raw: Some(object.object_raw),
                        object: object.object,
                        flags: 0u32,
                        create_time: 0u64,
                        update_time: 0u64,
                        insert_time: 0u64,
                        rank: OBJECT_RANK_NONE,
                    };
                    data.rebuild_object()?;

                    Some(data)
                } else {
                    None
                };

                let ret = NamedObjectCacheDeleteObjectResult {
                    deleted_count: 1,
                    object: object,
                };

                Ok(ret)
            }
            Err(e) => Err(e),
        }
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        unimplemented!();
    }

    fn sync_server(&self) -> Option<Box<dyn NamedObjectCacheSyncServer>> {
        unreachable!();
    }

    fn sync_client(&self) -> Option<Box<dyn NamedObjectCacheSyncClient>> {
        unreachable!();
    }

    fn clone_noc(&self) -> Box<dyn NamedObjectCache> {
        Box::new(self.clone()) as Box<dyn NamedObjectCache>
    }
}
