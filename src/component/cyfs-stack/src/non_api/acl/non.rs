use crate::acl::AclManagerRef;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::str::FromStr;
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

    async fn check_call_access(
        &self,
        req_path: &str,
        source: &RequestSourceInfo,
    ) -> BuckyResult<()> {
        let global_state_common = RequestGlobalStatePath::from_str(req_path)?;

        // 同zone+同dec，或者同zone+system，那么不需要校验rmeta权限
        if source.is_current_zone() {
            if source.check_target_dec_permission(&global_state_common.dec_id) {
                return Ok(());
            }
        }

        self.acl
            .global_state_meta()
            .check_access(source, &global_state_common, RequestOpType::Call).await
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
        if let Some(req_path) = &req.common.req_path {
            self.check_call_access(req_path, &req.common.source).await?;
        }

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
