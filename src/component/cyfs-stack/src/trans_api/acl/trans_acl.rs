use crate::trans::{TransInputProcessor, TransInputProcessorRef};
use cyfs_base::*;
use cyfs_core::TransContext;
use cyfs_lib::*;

use std::sync::Arc;

pub struct TransAclInnerInputProcessor {
    next: TransInputProcessorRef,
}

impl TransAclInnerInputProcessor {
    pub(crate) fn new(next: TransInputProcessorRef) -> TransInputProcessorRef {
        Arc::new(Box::new(Self { next }))
    }

    fn check_local_zone_permit(
        &self,
        service: &str,
        source: &RequestSourceInfo,
    ) -> BuckyResult<()> {
        if !source.is_current_zone() {
            let msg = format!(
                "{} service valid only in current zone! source={:?}, category={}",
                service,
                source.zone.device,
                source.zone.zone_category.as_str()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl TransInputProcessor for TransAclInnerInputProcessor {
    async fn get_context(&self, req: TransGetContextInputRequest) -> BuckyResult<TransContext> {
        self.check_local_zone_permit("get context", &req.common.source)?;
        self.next.get_context(req).await
    }

    async fn put_context(&self, req: TransUpdateContextInputRequest) -> BuckyResult<()> {
        self.check_local_zone_permit("update context", &req.common.source)?;
        self.next.put_context(req).await
    }

    async fn control_task(&self, req: TransControlTaskInputRequest) -> BuckyResult<()> {
        self.check_local_zone_permit("trans control task", &req.common.source)?;
        self.next.control_task(req).await
    }

    async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransTaskState> {
        self.check_local_zone_permit("trans get task state", &req.common.source)?;
        self.next.get_task_state(req).await
    }

    async fn publish_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        self.check_local_zone_permit("trans add file", &req.common.source)?;
        self.next.publish_file(req).await
    }

    async fn create_task(
        &self,
        req: TransCreateTaskInputRequest,
    ) -> BuckyResult<TransCreateTaskInputResponse> {
        self.check_local_zone_permit("trans create task", &req.common.source)?;
        self.next.create_task(req).await
    }

    async fn query_tasks(
        &self,
        req: TransQueryTasksInputRequest,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        self.check_local_zone_permit("trans query tasks", &req.common.source)?;
        self.next.query_tasks(req).await
    }
}
