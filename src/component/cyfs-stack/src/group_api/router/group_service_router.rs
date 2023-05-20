use std::sync::Arc;

use cyfs_base::{BuckyResult, DeviceId, ObjectId};
use cyfs_group_lib::{
    GroupPushProposalInputRequest, GroupPushProposalInputResponse,
    GroupRequestor, GroupStartServiceInputRequest, GroupStartServiceInputResponse,
};

use crate::{
    forward::ForwardProcessorManager,
    group::{GroupInputProcessor, GroupInputProcessorRef, GroupInputTransformer},
    group_api::GroupAclInnerInputProcessor,
    ZoneManagerRef,
};

#[derive(Clone)]
pub struct GroupServiceRouter {
    processor: GroupInputProcessorRef,
    forward: ForwardProcessorManager,
    zone_manager: ZoneManagerRef,
}

impl GroupServiceRouter {
    pub(crate) fn new(
        forward: ForwardProcessorManager,
        zone_manager: ZoneManagerRef,
        processor: GroupInputProcessorRef,
    ) -> GroupInputProcessorRef {
        let processor = GroupAclInnerInputProcessor::new(processor);
        let ret = Self {
            processor,
            zone_manager,
            forward,
        };
        Arc::new(ret)
    }

    async fn get_forward(
        &self,
        dec_id: ObjectId,
        target: DeviceId,
    ) -> BuckyResult<GroupInputProcessorRef> {
        let requestor = self.forward.get(&target).await?;
        let group_requestor = GroupRequestor::new(dec_id, requestor);
        Ok(GroupInputTransformer::new(
            group_requestor.clone_processor(),
        ))
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
        dec_id: ObjectId,
        target: Option<&ObjectId>,
    ) -> BuckyResult<GroupInputProcessorRef> {
        if let Some(device_id) = self.get_target(target).await? {
            debug!("group target resolved: {:?} -> {}", target, device_id);
            let processor = self.get_forward(dec_id, device_id).await?;
            Ok(processor)
        } else {
            Ok(self.processor.clone())
        }
    }

    pub async fn start_service(
        &self,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        let processor = self.get_processor(req.common.source.dec, None).await?;
        processor.start_service(req).await
    }

    pub async fn push_proposal(
        &self,
        req: GroupPushProposalInputRequest,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        let processor = self.get_processor(req.common.source.dec, None).await?;
        processor.push_proposal(req).await
    }
}

#[async_trait::async_trait]
impl GroupInputProcessor for GroupServiceRouter {
    async fn start_service(
        &self,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        let processor = self.get_processor(req.common.source.dec, None).await?;
        processor.start_service(req).await
    }

    async fn push_proposal(
        &self,
        req: GroupPushProposalInputRequest,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        let processor = self.get_processor(req.common.source.dec, None).await?;
        processor.push_proposal(req).await
    }
}
