use cyfs_base::*;
use cyfs_group_lib::{
    GroupPushProposalInputRequest, GroupPushProposalInputResponse, GroupStartServiceInputRequest,
    GroupStartServiceInputResponse,
};

use std::sync::Arc;

#[async_trait::async_trait]
pub(crate) trait GroupInputProcessor: Sync + Send {
    async fn start_service(
        &self,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse>;

    async fn push_proposal(
        &self,
        req: GroupPushProposalInputRequest,
    ) -> BuckyResult<GroupPushProposalInputResponse>;
}

pub(crate) type GroupInputProcessorRef = Arc<dyn GroupInputProcessor>;
