use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NONInputAclSwitcher {
    acl_processor: NONInputProcessorRef,
    raw_processor: NONInputProcessorRef,
}

impl NONInputAclSwitcher {
    pub fn new(
        acl_processor: NONInputProcessorRef,
        raw_processor: NONInputProcessorRef,
    ) -> NONInputProcessorRef {
        let ret = Self {
            acl_processor,
            raw_processor,
        };

        Arc::new(Box::new(ret))
    }

    pub fn is_require_acl(&self, req_common: &NONInputRequestCommon) -> bool {
        req_common.protocol.is_require_acl()
    }

    pub fn get_processor(&self, req_common: &NONInputRequestCommon) -> &NONInputProcessorRef {
        match self.is_require_acl(req_common) {
            true => &self.acl_processor,
            false => &self.raw_processor,
        }
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONInputAclSwitcher {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        self.get_processor(&req.common).put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        self.get_processor(&req.common).get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        self.get_processor(&req.common).post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        self.get_processor(&req.common).select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        self.get_processor(&req.common).delete_object(req).await
    }
}
