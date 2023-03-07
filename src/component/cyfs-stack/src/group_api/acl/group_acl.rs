use crate::group::{GroupInputProcessor, GroupInputProcessorRef};
use cyfs_base::*;
use cyfs_core::GroupProposal;
use cyfs_group_lib::{
    GroupPushProposalInputResponse, GroupStartServiceInputRequest, GroupStartServiceInputResponse,
};
use cyfs_lib::*;

use std::sync::Arc;

pub struct GroupAclInnerInputProcessor {
    next: GroupInputProcessorRef,
}

impl GroupAclInnerInputProcessor {
    pub(crate) fn new(next: GroupInputProcessorRef) -> GroupInputProcessorRef {
        Arc::new(Self { next })
    }

    fn check_local_zone_permit(
        &self,
        service: &str,
        source: &RequestSourceInfo,
    ) -> BuckyResult<()> {
        // TODO
        // if !source.is_current_zone() {
        //     let msg = format!(
        //         "{} service valid only in current zone! source={:?}, category={}",
        //         service,
        //         source.zone.device,
        //         source.zone.zone_category.as_str()
        //     );
        //     error!("{}", msg);

        //     return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        // }

        Ok(())
    }
}

#[async_trait::async_trait]
impl GroupInputProcessor for GroupAclInnerInputProcessor {
    async fn start_service(
        &self,
        req_common: NONInputRequestCommon,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        self.check_local_zone_permit("group.start-service", &req_common.source)?;
        self.next.start_service(req_common, req).await
    }

    async fn push_proposal(
        &self,
        req_common: NONInputRequestCommon,
        req: GroupProposal,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        self.check_local_zone_permit("group.push-proposal", &req_common.source)?;
        self.next.push_proposal(req_common, req).await
    }
}
