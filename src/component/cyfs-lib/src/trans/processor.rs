use crate::*;
use cyfs_base::BuckyResult;
use cyfs_core::TransContext;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait TransOutputProcessor: Send + Sync {
    async fn get_context(&self, req: TransGetContextOutputRequest) -> BuckyResult<TransContext>;
    async fn put_context(&self, req: TransPutContextOutputRequest) -> BuckyResult<()>;
    async fn create_task(
        &self,
        req: TransCreateTaskOutputRequest,
    ) -> BuckyResult<TransCreateTaskOutputResponse>;
    async fn control_task(&self, req: TransControlTaskOutputRequest) -> BuckyResult<()>;
    async fn query_tasks(
        &self,
        req: TransQueryTasksOutputRequest,
    ) -> BuckyResult<TransQueryTasksOutputResponse>;
    async fn get_task_state(
        &self,
        req: TransGetTaskStateOutputRequest,
    ) -> BuckyResult<TransGetTaskStateOutputResponse>;
    async fn publish_file(
        &self,
        req: TransPublishFileOutputRequest,
    ) -> BuckyResult<TransPublishFileOutputResponse>;

    // task group
    async fn get_task_group_state(
        &self,
        req: TransGetTaskGroupStateOutputRequest,
    ) -> BuckyResult<TransGetTaskGroupStateOutputResponse>;
    async fn control_task_group(
        &self,
        req: TransControlTaskGroupOutputRequest,
    ) -> BuckyResult<TransControlTaskGroupOutputResponse>;
}

pub type TransOutputProcessorRef = Arc<dyn TransOutputProcessor>;
