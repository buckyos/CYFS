use crate::group::{GroupInputProcessor, GroupInputProcessorRef};
use cyfs_base::*;
use cyfs_group_lib::{
    GroupPushProposalInputRequest, GroupPushProposalInputResponse,
    GroupStartServiceInputRequest, GroupStartServiceInputResponse,
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
        _service: &str,
        _source: &RequestSourceInfo,
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
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        self.check_local_zone_permit("group.service", &req.common.source)?;
        self.next.start_service(req).await
    }

    async fn push_proposal(
        &self,
        req: GroupPushProposalInputRequest,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        self.check_local_zone_permit("group.proposal", &req.common.source)?;
        self.next.push_proposal(req).await
    }
}
