use crate::acl::*;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;


// 限定在同zone内操作
pub(crate) struct NONAclInnerInputProcessor {
    acl: AclManagerRef,
    next: NONInputProcessorRef,
}

impl NONAclInnerInputProcessor {
    pub fn new_raw(acl: AclManagerRef, next: NONInputProcessorRef) -> Self {
        Self { acl, next }
    }

    pub fn new(acl: AclManagerRef, next: NONInputProcessorRef) -> NONInputProcessorRef {
        let ret = Self::new_raw(acl, next);
        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONAclInnerInputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        self.acl
            .check_local_zone_permit("non in put_object", &req.common.source)
            .await?;

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        self.acl
            .check_local_zone_permit("non in get_object", &req.common.source)
            .await?;

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        self.acl
            .check_local_zone_permit("non in post_object", &req.common.source)
            .await?;

        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        // TODO select暂时只允许同zone内使用
        self.acl
            .check_local_zone_permit("non in select_object", &req.common.source)
            .await?;

        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        self.acl
            .check_local_zone_permit("non in delete_object", &req.common.source)
            .await?;

        self.next.delete_object(req).await
    }
}

pub(crate) struct NONAclInnerOutputProcessor {
    acl: AclManagerRef,
    target: DeviceId,
    next: NONOutputProcessorRef,
}

impl NONAclInnerOutputProcessor {
    pub fn new(acl: AclManagerRef, target: DeviceId, next: NONOutputProcessorRef) -> NONOutputProcessorRef {
        let ret = Self {
            acl,
            target,
            next,
        };

        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl NONOutputProcessor for NONAclInnerOutputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        self.acl
            .check_local_zone_permit("non out put_object", &self.target)
            .await?;

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectOutputRequest,
    ) -> BuckyResult<NONGetObjectOutputResponse> {
        self.acl
            .check_local_zone_permit("non out get_object", &self.target)
            .await?;

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectOutputRequest,
    ) -> BuckyResult<NONPostObjectOutputResponse> {
        self.acl
            .check_local_zone_permit("non out post_object", &self.target)
            .await?;

        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectOutputRequest,
    ) -> BuckyResult<NONSelectObjectOutputResponse> {
        // 对于output请求，select暂时不做限制
        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectOutputRequest,
    ) -> BuckyResult<NONDeleteObjectOutputResponse> {
        self.acl
            .check_local_zone_permit("non out delete_object", &self.target)
            .await?;

        self.next.delete_object(req).await
    }
}
