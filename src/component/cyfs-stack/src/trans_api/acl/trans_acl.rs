use crate::trans::{TransInputProcessor, TransInputProcessorRef};
use crate::AclManagerRef;
use cyfs_base::BuckyResult;
use cyfs_core::TransContext;
use cyfs_lib::*;
use std::sync::Arc;

pub struct TransAclInnerInputProcessor {
    acl: AclManagerRef,
    next: TransInputProcessorRef,
}

impl TransAclInnerInputProcessor {
    pub(crate) fn new(acl: AclManagerRef, next: TransInputProcessorRef) -> TransInputProcessorRef {
        Arc::new(Self { acl, next })
    }
}

#[async_trait::async_trait]
impl TransInputProcessor for TransAclInnerInputProcessor {
    async fn get_context(&self, req: TransGetContextInputRequest) -> BuckyResult<TransContext> {
        self.acl
            .check_local_zone_permit("get context", &req.common.source)
            .await?;
        self.next.get_context(req).await
    }

    async fn put_context(&self, req: TransUpdateContextInputRequest) -> BuckyResult<()> {
        self.acl
            .check_local_zone_permit("update context", &req.common.source)
            .await?;
        self.next.put_context(req).await
    }

    async fn control_task(&self, req: TransControlTaskInputRequest) -> BuckyResult<()> {
        self.acl
            .check_local_zone_permit("trans control task", &req.common.source)
            .await?;
        self.next.control_task(req).await
    }

    async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransTaskState> {
        self.acl
            .check_local_zone_permit("trans get task state", &req.common.source)
            .await?;
        self.next.get_task_state(req).await
    }

    async fn publish_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        self.acl
            .check_local_zone_permit("trans add file", &req.common.source)
            .await?;
        self.next.publish_file(req).await
    }

    async fn create_task(
        &self,
        req: TransCreateTaskInputRequest,
    ) -> BuckyResult<TransCreateTaskInputResponse> {
        self.acl
            .check_local_zone_permit("trans create task", &req.common.source)
            .await?;
        self.next.create_task(req).await
    }

    async fn query_tasks(
        &self,
        req: TransQueryTasksInputRequest,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        self.acl
            .check_local_zone_permit("trans query tasks", &req.common.source)
            .await?;
        self.next.query_tasks(req).await
    }
}
