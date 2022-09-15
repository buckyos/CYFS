use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 限定在同zone内操作
pub(crate) struct NONZoneAclInputProcessor {
    next: NONInputProcessorRef,
}

impl NONZoneAclInputProcessor {
    pub fn new_raw(next: NONInputProcessorRef) -> Self {
        Self { next }
    }

    pub fn new(next: NONInputProcessorRef) -> NONInputProcessorRef {
        let ret = Self::new_raw(next);
        Arc::new(Box::new(ret))
    }

    fn check_access(&self, service: &str, common: &NONInputRequestCommon) -> BuckyResult<()> {
        common.source.check_current_zone(service)
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONZoneAclInputProcessor {
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
        // TODO select暂时只允许同zone内使用
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
