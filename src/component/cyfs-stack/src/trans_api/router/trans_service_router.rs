use crate::meta::ObjectFailHandler;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

use crate::forward::ForwardProcessorManager;
use crate::trans::{TransInputProcessor, TransInputProcessorRef, TransInputTransformer};
use crate::trans_api::TransAclInnerInputProcessor;
use crate::zone::ZoneManagerRef;

#[derive(Clone)]
pub struct TransServiceRouter {
    processor: TransInputProcessorRef,
    forward: ForwardProcessorManager,
    fail_handler: ObjectFailHandler,
    zone_manager: ZoneManagerRef,
}

impl TransServiceRouter {
    pub(crate) fn new(
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        fail_handler: ObjectFailHandler,
        processor: TransInputProcessorRef,
    ) -> TransInputProcessorRef {
        let processor = TransAclInnerInputProcessor::new(processor);
        let ret = Self {
            processor,
            zone_manager,
            forward,
            fail_handler,
        };
        Arc::new(Box::new(ret))
    }

    async fn get_forward(&self, target: DeviceId) -> BuckyResult<TransInputProcessorRef> {
        let requestor = self.forward.get(&target).await?;
        let trans_requestor = TransRequestor::new(None, requestor);
        let processor = Arc::new(trans_requestor);
        Ok(TransInputTransformer::new(processor))
    }

    // 不同于non/ndn的router，如果target为空，那么表示本地device
    async fn get_target(&self, target: Option<&ObjectId>) -> BuckyResult<Option<DeviceId>> {
        let ret = match target {
            Some(object_id) => {
                let info = self
                    .zone_manager
                    .target_zone_manager()
                    .resolve_target(Some(object_id))
                    .await?;
                if info.target_device == *self.zone_manager.get_current_device_id() {
                    None
                } else {
                    Some(info.target_device)
                }
            }
            None => None,
        };

        Ok(ret)
    }

    async fn get_processor(
        &self,
        target: Option<&ObjectId>,
    ) -> BuckyResult<TransInputProcessorRef> {
        if let Some(device_id) = self.get_target(target).await? {
            debug!("trans target resolved: {:?} -> {}", target, device_id);
            let processor = self.get_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.processor.clone())
        }
    }

    pub async fn create_task(
        &self,
        req: TransCreateTaskInputRequest,
    ) -> BuckyResult<TransCreateTaskInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.create_task(req).await
    }

    pub async fn control_task(&self, req: TransControlTaskInputRequest) -> BuckyResult<()> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.control_task(req).await
    }

    async fn query_tasks(
        &self,
        req: TransQueryTasksInputRequest,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.query_tasks(req).await
    }

    pub async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransGetTaskStateInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_task_state(req).await
    }

    pub async fn publish_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        if req.common.target.is_some() {
            let msg = format!("target not support for trans.publish_file!");
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
        }

        self.processor.publish_file(req).await
    }

    async fn get_task_group_state(
        &self,
        req: TransGetTaskGroupStateInputRequest,
    ) -> BuckyResult<TransGetTaskGroupStateInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_task_group_state(req).await
    }

    async fn control_task_group(
        &self,
        req: TransControlTaskGroupInputRequest,
    ) -> BuckyResult<TransControlTaskGroupInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.control_task_group(req).await
    }
}

#[async_trait::async_trait]
impl TransInputProcessor for TransServiceRouter {
    async fn get_context(&self, req: TransGetContextInputRequest) -> BuckyResult<TransGetContextInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_context(req).await
    }

    async fn put_context(&self, req: TransUpdateContextInputRequest) -> BuckyResult<()> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.put_context(req).await
    }

    async fn create_task(
        &self,
        req: TransCreateTaskInputRequest,
    ) -> BuckyResult<TransCreateTaskInputResponse> {
        Self::create_task(self, req).await
    }

    async fn control_task(&self, req: TransControlTaskInputRequest) -> BuckyResult<()> {
        Self::control_task(self, req).await
    }

    async fn query_tasks(
        &self,
        req: TransQueryTasksInputRequest,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        Self::query_tasks(self, req).await
    }

    async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransGetTaskStateInputResponse> {
        Self::get_task_state(self, req).await
    }

    async fn publish_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        Self::publish_file(self, req).await
    }

    async fn get_task_group_state(
        &self,
        req: TransGetTaskGroupStateInputRequest,
    ) -> BuckyResult<TransGetTaskGroupStateInputResponse> {
        Self::get_task_group_state(self, req).await
    }

    async fn control_task_group(
        &self,
        req: TransControlTaskGroupInputRequest,
    ) -> BuckyResult<TransControlTaskGroupInputResponse> {
        Self::control_task_group(self, req).await
    }
}
