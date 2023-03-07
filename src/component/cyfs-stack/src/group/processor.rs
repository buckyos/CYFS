use cyfs_base::*;
use cyfs_core::GroupProposal;
use cyfs_group_lib::{
    GroupPushProposalInputResponse, GroupStartServiceInputRequest, GroupStartServiceInputResponse,
};
use cyfs_lib::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub(crate) trait GroupInputProcessor: Sync + Send {
    async fn start_service(
        &self,
        req_common: NONInputRequestCommon,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse>;

    async fn push_proposal(
        &self,
        req_common: NONInputRequestCommon,
        req: GroupProposal,
    ) -> BuckyResult<GroupPushProposalInputResponse>;
}

pub(crate) type GroupInputProcessorRef = Arc<dyn GroupInputProcessor>;
