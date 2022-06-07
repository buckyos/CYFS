use crate::acl::*;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;


// 限定在同device协议栈内操作
pub(crate) struct NONAclLocalInputProcessor {
    acl: AclManagerRef,
    next: NONInputProcessorRef,
}

impl NONAclLocalInputProcessor {
    pub fn new(acl: AclManagerRef, next: NONInputProcessorRef) -> NONInputProcessorRef {
        let ret = Self { acl, next };
        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONAclLocalInputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        self.acl
            .check_local_permit("non in put_object", &req.common.source)
            .await?;

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        self.acl
            .check_local_permit("non in get_object", &req.common.source)
            .await?;

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        self.acl
            .check_local_permit("non in post_object", &req.common.source)
            .await?;

        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        self.acl
            .check_local_permit("non in select_object", &req.common.source)
            .await?;

        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        self.acl
            .check_local_permit("non in delete_object", &req.common.source)
            .await?;

        self.next.delete_object(req).await
    }
}

pub(crate) struct NONAclLocalOutputProcessor {
    acl: AclManagerRef,
    target: DeviceId,
    next: NONOutputProcessorRef,
}

impl NONAclLocalOutputProcessor {
    pub fn new(acl: AclManagerRef, target: DeviceId, next: NONOutputProcessorRef) -> Self {
        Self {
            acl,
            target,
            next,
        }
    }
}

#[async_trait::async_trait]
impl NONOutputProcessor for NONAclLocalOutputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        self.acl
            .check_local_permit("non out put_object", &self.target)
            .await?;

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectOutputRequest,
    ) -> BuckyResult<NONGetObjectOutputResponse> {
        self.acl
            .check_local_permit("non out get_object", &self.target)
            .await?;

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectOutputRequest,
    ) -> BuckyResult<NONPostObjectOutputResponse> {
        self.acl
            .check_local_permit("non out post_object", &self.target)
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
            .check_local_permit("non out delete_object", &self.target)
            .await?;

        self.next.delete_object(req).await
    }
}
