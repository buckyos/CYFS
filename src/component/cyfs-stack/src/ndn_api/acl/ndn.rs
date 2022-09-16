use crate::acl::*;
use crate::ndn::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NDNAclInputProcessor {
    acl: AclManagerRef,
    next: NDNInputProcessorRef,
}

impl NDNAclInputProcessor {
    pub fn new(acl: AclManagerRef, next: NDNInputProcessorRef) -> NDNInputProcessorRef {
        let ret = Self { acl, next };
        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNAclInputProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!(
                "put_data only allow within the same zone! {}",
                req.object_id
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        // FIXME 设计合理的权限，需要配合object_id和referer_objects
        warn!(">>>>>>>>>>>>>>>>>>>>>>get_data acl not impl!!!!");

        self.next.get_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!(
                "delete_data only allow within the same zone! {}",
                req.object_id
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!(
                "query_file only allow within the same zone! {}",
                req
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.query_file(req).await
    }
}