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
            .check_access(source, &global_state_common, op_type)
            .await
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONGlobalStateMetaAclInputProcessor {
    async fn put_object(
        &self,
        mut req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            self.check_access(req_path, &req.common.source, RequestOpType::Write)
                .await?;
            req.common.source.set_verified();
        }

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        mut req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            self.check_access(req_path, &req.common.source, RequestOpType::Read)
                .await?;
            req.common.source.set_verified();
        }

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        mut req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            self.check_access(req_path, &req.common.source, RequestOpType::Call)
                .await?;
            req.common.source.set_verified();
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
            self.check_access(req_path, &req.common.source, RequestOpType::Write)
                .await?;
            req.common.source.set_verified();
        }

        self.next.delete_object(req).await
    }
}
