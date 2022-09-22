use cyfs_base::BuckyResult;
use cyfs_core::TransContext;
use cyfs_lib::*;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait TransInputProcessor: Send + Sync {
    async fn get_context(&self, req: TransGetContextInputRequest) -> BuckyResult<TransContext>;
    async fn put_context(&self, req: TransUpdateContextInputRequest) -> BuckyResult<()>;
    async fn create_task(
        &self,
        req: TransCreateTaskInputRequest,
    ) -> BuckyResult<TransCreateTaskInputResponse>;
    async fn control_task(&self, req: TransControlTaskInputRequest) -> BuckyResult<()>;
    async fn query_tasks(
        &self,
        req: TransQueryTasksInputRequest,
    ) -> BuckyResult<TransQueryTasksInputResponse>;
    async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransTaskState>;
    async fn publish_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse>;
}
pub type TransInputProcessorRef = Arc<Box<dyn TransInputProcessor>>;
