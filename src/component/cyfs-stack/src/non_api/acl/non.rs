use crate::acl::AclManagerRef;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NONAclInputProcessor {
    acl: AclManagerRef,
    next: NONInputProcessorRef,
}

impl NONAclInputProcessor {
    pub fn new(acl: AclManagerRef, next: NONInputProcessorRef) -> NONInputProcessorRef {
        let ret = Self { acl, next };
        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONAclInputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!(
                "put_object only allow within the same zone! {}",
                req.object.object_id
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
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
        if !req.common.source.is_current_zone() {
            let msg = format!("select_object only allow within the same zone! {}", req);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!(
                "delete_object only allow within the same zone! {}",
                req.object_id
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.delete_object(req).await
    }
}
