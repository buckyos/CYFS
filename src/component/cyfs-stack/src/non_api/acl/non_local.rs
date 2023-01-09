use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 限定在同device协议栈内操作
pub(crate) struct NONLocalAclInputProcessor {
    next: NONInputProcessorRef,
}

impl NONLocalAclInputProcessor {
    pub fn new(next: NONInputProcessorRef) -> NONInputProcessorRef {
        let ret = Self { next };
        Arc::new(Box::new(ret))
    }

    fn check_access(&self, service: &str, common: &NONInputRequestCommon) -> BuckyResult<()> {
        common.source.check_current_device(service)
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONLocalAclInputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        self.check_access("non.put_object", &req.common)?;

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        self.check_access("non.get_object", &req.common)?;

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        self.check_access("non.post_object", &req.common)?;

        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        self.check_access("non.select_object", &req.common)?;

        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        self.check_access("non.delete_object", &req.common)?;

        self.next.delete_object(req).await
    }
}
