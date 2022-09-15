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
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        if !req.common.source.is_current_zone() {
            // FIXME put_object should use the rmeta acl system?
            if let Some(req_path) = &req.common.req_path {
                self.check_access(req_path, &req.common.source, RequestOpType::Write)
                    .await?;
            }
        }

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        if !req.common.source.is_current_zone() {
            if let Some(req_path) = &req.common.req_path {
                self.check_access(req_path, &req.common.source, RequestOpType::Read)
                    .await?;
            }
        }

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        if !req.common.source.is_current_zone() {
            if let Some(req_path) = &req.common.req_path {
                self.check_access(req_path, &req.common.source, RequestOpType::Call)
                    .await?;
            }
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
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        if !req.common.source.is_current_zone() {
            if let Some(req_path) = &req.common.req_path {
                self.check_access(req_path, &req.common.source, RequestOpType::Write)
                    .await?;
            }
        }

        self.next.delete_object(req).await
    }
}
