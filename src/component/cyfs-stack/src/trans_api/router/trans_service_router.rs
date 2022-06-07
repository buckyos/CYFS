use crate::meta::ObjectFailHandler;
use cyfs_base::*;
use cyfs_core::TransContext;
use cyfs_lib::*;
use std::sync::Arc;

use crate::forward::ForwardProcessorManager;
use crate::trans::{TransInputProcessor, TransInputProcessorRef, TransInputTransformer};
use crate::trans_api::TransAclInnerInputProcessor;
use crate::zone::ZoneManager;
use crate::AclManagerRef;

pub struct TransServiceRouter {
    processor: TransInputProcessorRef,
    forward: ForwardProcessorManager,
    fail_handler: ObjectFailHandler,
    zone_manager: ZoneManager,
}

impl Clone for TransServiceRouter {
    fn clone(&self) -> Self {
        Self {
            processor: self.processor.clone(),
            forward: self.forward.clone(),
            fail_handler: self.fail_handler.clone(),
            zone_manager: self.zone_manager.clone(),
        }
    }
}

impl TransServiceRouter {
    pub(crate) fn new(
        acl: AclManagerRef,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManager,
        fail_handler: ObjectFailHandler,
        processor: TransInputProcessorRef,
    ) -> Self {
        let processor = TransAclInnerInputProcessor::new(acl, processor);
        Self {
            processor,
            zone_manager,
            forward,
            fail_handler,
        }
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
                let (_, device_id) = self
                    .zone_manager
                    .resolve_target(Some(object_id), None)
                    .await?;
                if device_id == *self.zone_manager.get_current_device_id() {
                    None
                } else {
                    Some(device_id)
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
            debug!("util target resolved: {:?} -> {}", target, device_id);
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

    pub async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransTaskState> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_task_state(req).await
    }

    pub async fn add_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        self.processor.publish_file(req).await
    }
}

#[async_trait::async_trait]
impl TransInputProcessor for TransServiceRouter {
    async fn get_context(&self, req: TransGetContextInputRequest) -> BuckyResult<TransContext> {
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
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.query_tasks(req).await
    }

    async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransTaskState> {
        Self::get_task_state(self, req).await
    }

    async fn publish_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        Self::add_file(self, req).await
    }
}
