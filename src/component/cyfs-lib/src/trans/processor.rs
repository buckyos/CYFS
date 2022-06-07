use crate::*;
use cyfs_base::BuckyResult;
use cyfs_core::TransContext;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait TransOutputProcessor: Send + Sync {
    async fn get_context(&self, req: &TransGetContextOutputRequest) -> BuckyResult<TransContext>;
    async fn put_context(&self, req: &TransPutContextOutputRequest) -> BuckyResult<()>;
    async fn create_task(
        &self,
        req: &TransCreateTaskOutputRequest,
    ) -> BuckyResult<TransCreateTaskOutputResponse>;
    async fn start_task(&self, req: &TransTaskOutputRequest) -> BuckyResult<()>;
    async fn stop_task(&self, req: &TransTaskOutputRequest) -> BuckyResult<()>;
    async fn delete_task(&self, req: &TransTaskOutputRequest) -> BuckyResult<()>;
    async fn query_tasks(
        &self,
        req: &TransQueryTasksOutputRequest,
    ) -> BuckyResult<TransQueryTasksOutputResponse>;
    async fn get_task_state(
        &self,
        req: &TransGetTaskStateOutputRequest,
    ) -> BuckyResult<TransTaskState>;
    async fn publish_file(
        &self,
        req: &TransPublishFileOutputRequest,
    ) -> BuckyResult<TransPublishFileOutputResponse>;
}
pub type TransOutputProcessorRef = Arc<dyn TransOutputProcessor>;
