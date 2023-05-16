use std::sync::Arc;

use cyfs_base::BuckyResult;
use cyfs_core::GroupProposal;

use crate::{
    GroupOutputRequestCommon, GroupPushProposalOutputResponse, GroupStartServiceOutputRequest,
    GroupStartServiceOutputResponse,
};

#[async_trait::async_trait]
pub trait GroupOutputProcessor: Send + Sync {
    async fn start_service(
        &self,
        req_common: GroupOutputRequestCommon,
        req: GroupStartServiceOutputRequest,
    ) -> BuckyResult<GroupStartServiceOutputResponse>;
    async fn push_proposal(
        &self,
        req_common: GroupOutputRequestCommon,
        req: GroupProposal,
    ) -> BuckyResult<GroupPushProposalOutputResponse>;
}

pub type GroupOutputProcessorRef = Arc<Box<dyn GroupOutputProcessor>>;
