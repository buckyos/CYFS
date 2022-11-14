use super::super::acl::GlobalStateMetaAclInnerInputProcessor;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::rmeta::*;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub struct GlobalStateMetaServiceRouter {
    category: GlobalStateCategory,
    processor: GlobalStateMetaInputProcessorRef,
    forward: ForwardProcessorManager,
    fail_handler: ObjectFailHandler,
    zone_manager: ZoneManagerRef,
}

impl Clone for GlobalStateMetaServiceRouter {
    fn clone(&self) -> Self {
        Self {
            category: self.category.clone(),
            processor: self.processor.clone(),
            forward: self.forward.clone(),
            fail_handler: self.fail_handler.clone(),
            zone_manager: self.zone_manager.clone(),
        }
    }
}

impl GlobalStateMetaServiceRouter {
    pub(crate) fn new(
        category: GlobalStateCategory,
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        fail_handler: ObjectFailHandler,
        processor: GlobalStateMetaInputProcessorRef,
    ) -> Self {
        let processor = GlobalStateMetaAclInnerInputProcessor::new(processor);
        Self {
            category,
            processor,
            zone_manager,
            forward,
            fail_handler,
        }
    }

    pub fn clone_processor(&self) -> GlobalStateMetaInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    async fn get_forward(&self, target: DeviceId) -> BuckyResult<GlobalStateMetaInputProcessorRef> {
        let requestor = self.forward.get(&target).await?;
        let requestor = GlobalStateMetaRequestor::new(self.category, None, requestor);

        // 转换为input processor
        let input_processor = GlobalStateMetaInputTransformer::new(requestor.into_processor());

        Ok(input_processor)
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
    ) -> BuckyResult<GlobalStateMetaInputProcessorRef> {
        if let Some(device_id) = self.get_target(target).await? {
            debug!(
                "global state meta target resolved: {:?} -> {}",
                target, device_id
            );
            let processor = self.get_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.processor.clone())
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateMetaInputProcessor for GlobalStateMetaServiceRouter {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.add_access(req).await
    }

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.remove_access(req).await
    }

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.clear_access(req).await
    }

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.add_link(req).await
    }

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.remove_link(req).await
    }

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.clear_link(req).await
    }

    async fn add_object_meta(
        &self,
        req: GlobalStateMetaAddObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddObjectMetaInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.add_object_meta(req).await
    }

    async fn remove_object_meta(
        &self,
        req: GlobalStateMetaRemoveObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveObjectMetaInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.remove_object_meta(req).await
    }

    async fn clear_object_meta(
        &self,
        req: GlobalStateMetaClearObjectMetaInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearObjectMetaInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.clear_object_meta(req).await
    }
}
