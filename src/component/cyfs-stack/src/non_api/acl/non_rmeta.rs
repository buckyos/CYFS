use crate::acl::AclManagerRef;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::str::FromStr;
use std::sync::Arc;

pub(crate) struct NONGlobalStateMetaAclInputProcessor {
    acl: AclManagerRef,
    next: NONInputProcessorRef,
}

impl NONGlobalStateMetaAclInputProcessor {
    pub fn new(acl: AclManagerRef, next: NONInputProcessorRef) -> NONInputProcessorRef {
        let ret = Self { acl, next };
        Arc::new(Box::new(ret))
    }

    async fn check_access(
        &self,
        req_path: &str,
        source: &RequestSourceInfo,
        op_type: RequestOpType,
    ) -> BuckyResult<ObjectId> {
        debug!("will check access: req_path={}, source={}, {:?}", req_path, source, op_type);

        let req_path = RequestGlobalStatePath::from_str(req_path)?;

        // 同zone+同dec，或者同zone+system，那么不需要校验rmeta权限
        if source.is_current_zone() {
            if source.check_target_dec_permission(&req_path.dec_id) {
                return Ok(req_path.dec(source).to_owned());
            }
        }

        self.acl
            .global_state_meta()
            .check_access(source, &req_path, op_type)
            .await?;

        Ok(req_path.dec(source).to_owned())
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONGlobalStateMetaAclInputProcessor {
    async fn put_object(
        &self,
        mut req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            let dec_id = self.check_access(req_path, &req.common.source, RequestOpType::Write)
                .await?;
            req.common.source.set_verified(dec_id);
        }

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        mut req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            let dec_id = self.check_access(req_path, &req.common.source, RequestOpType::Read)
                .await?;
            req.common.source.set_verified(dec_id);
        }

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        mut req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            let dec_id = self.check_access(req_path, &req.common.source, RequestOpType::Call)
                .await?;
            req.common.source.set_verified(dec_id);
        } else {
            let msg = format!("post_object must specify a valid req_path field! object={}", req.object.object_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

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
        mut req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            let dec_id = self.check_access(req_path, &req.common.source, RequestOpType::Write)
                .await?;
            req.common.source.set_verified(dec_id);
        }

        self.next.delete_object(req).await
    }
}
