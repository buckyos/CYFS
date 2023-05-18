use std::sync::Arc;

use cyfs_base::BuckyResult;
use cyfs_core::GroupProposal;

use crate::{
    GroupOutputRequestCommon, GroupPushProposalOutputRequest, GroupPushProposalOutputResponse,
    GroupStartServiceOutputRequest, GroupStartServiceOutputResponse,
};

#[async_trait::async_trait]
pub trait GroupOutputProcessor: Send + Sync {
    async fn start_service(
        &self,
        req: GroupStartServiceOutputRequest,
    ) -> BuckyResult<GroupStartServiceOutputResponse>;
    async fn push_proposal(
        &self,
        req: GroupPushProposalOutputRequest,
    ) -> BuckyResult<GroupPushProposalOutputResponse>;
}

pub type GroupOutputProcessorRef = Arc<Box<dyn GroupOutputProcessor>>;
