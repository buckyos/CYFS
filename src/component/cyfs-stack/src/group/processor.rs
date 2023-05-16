use cyfs_base::*;
use cyfs_core::GroupProposal;
use cyfs_group_lib::{
    GroupInputRequestCommon, GroupPushProposalInputResponse, GroupStartServiceInputRequest,
    GroupStartServiceInputResponse,
};

use std::sync::Arc;

#[async_trait::async_trait]
pub(crate) trait GroupInputProcessor: Sync + Send {
    async fn start_service(
        &self,
        req_common: GroupInputRequestCommon,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse>;

    async fn push_proposal(
        &self,
        req_common: GroupInputRequestCommon,
        req: GroupProposal,
    ) -> BuckyResult<GroupPushProposalInputResponse>;
}

pub(crate) type GroupInputProcessorRef = Arc<dyn GroupInputProcessor>;
