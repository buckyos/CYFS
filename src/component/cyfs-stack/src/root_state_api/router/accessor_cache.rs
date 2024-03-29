use crate::non::*;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub struct GlobalStateAccessorCacheProcessor {
    next: GlobalStateAccessorInputProcessorRef,

    device_id: DeviceId,

    noc: NONInputProcessorRef,
}

impl GlobalStateAccessorCacheProcessor {
    pub(crate) fn new(
        next: GlobalStateAccessorInputProcessorRef,
        noc: NONInputProcessorRef,
        device_id: DeviceId,
    ) -> GlobalStateAccessorInputProcessorRef {
        let ret = Self {
            next,
            noc,
            device_id,
        };

        Arc::new(Box::new(ret))
    }

    pub async fn cache_object(
        &self,
        req: RootStateAccessorGetObjectByPathInputRequest,
        object: &NONObjectInfo,
    ) {
        let put_req = NONPutObjectInputRequest {
            common: NONInputRequestCommon {
                req_path: Some(req.inner_path),
                source: req.common.source,
                level: NONAPILevel::NOC,
                target: None,
                flags: 0,
            },
            object: object.clone(),
            access: None,
        };

        let _r = self.noc.put_object(put_req).await;
    }
}

#[async_trait::async_trait]
impl GlobalStateAccessorInputProcessor for GlobalStateAccessorCacheProcessor {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessorGetObjectByPathInputRequest,
    ) -> BuckyResult<RootStateAccessorGetObjectByPathInputResponse> {
        let cache_req = req.clone();
        let resp = self.next.get_object_by_path(req).await?;

        // FIXME now only cache file
        match resp.object.object.object_id.obj_type_code() {
            ObjectTypeCode::File => {
                let _ = self.cache_object(cache_req, &resp.object.object).await;
            }
            _ => {}
        }

        Ok(resp)
    }

    async fn list(
        &self,
        req: RootStateAccessorListInputRequest,
    ) -> BuckyResult<RootStateAccessorListInputResponse> {
        self.next.list(req).await
    }
}
