use cyfs_base::*;
use cyfs_core::GroupProposal;
use cyfs_group_lib::{
    GroupInputRequestCommon, GroupOutputProcessor, GroupOutputProcessorRef,
    GroupOutputRequestCommon, GroupPushProposalInputRequest, GroupPushProposalInputResponse,
    GroupPushProposalOutputRequest, GroupPushProposalOutputResponse, GroupStartServiceInputRequest,
    GroupStartServiceInputResponse, GroupStartServiceOutputRequest,
    GroupStartServiceOutputResponse,
};
use cyfs_lib::*;

use std::sync::Arc;

use super::{GroupInputProcessor, GroupInputProcessorRef};

// 实现从input到output的转换
pub(crate) struct GroupInputTransformer {
    processor: GroupOutputProcessorRef,
}

impl GroupInputTransformer {
    pub fn new(processor: GroupOutputProcessorRef) -> GroupInputProcessorRef {
        let ret = Self { processor };
        Arc::new(ret)
    }

    fn convert_common(common: GroupInputRequestCommon) -> GroupOutputRequestCommon {
        GroupOutputRequestCommon {
            dec_id: common.source.get_opt_dec().cloned(),
        }
    }

    async fn start_service(
        &self,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        let out_req = GroupStartServiceOutputRequest {
            group_id: req.group_id,
            rpath: req.rpath,
            common: Self::convert_common(req.common),
        };

        let out_resp = self.processor.start_service(out_req).await?;

        let resp = GroupStartServiceInputResponse {};

        Ok(resp)
    }

    async fn push_proposal(
        &self,
        req: GroupPushProposalInputRequest,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        let out_req = GroupPushProposalOutputRequest {
            proposal: req.proposal,
            common: Self::convert_common(req.common),
        };

        let out_resp = self.processor.push_proposal(out_req).await?;

        let resp = GroupPushProposalInputResponse {
            object: out_resp.object,
        };

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl GroupInputProcessor for GroupInputTransformer {
    async fn start_service(
        &self,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        GroupInputTransformer::start_service(self, req).await
    }

    async fn push_proposal(
        &self,
        req: GroupPushProposalInputRequest,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        GroupInputTransformer::push_proposal(self, req).await
    }
}

// 实现从output到input的转换
pub(crate) struct GroupOutputTransformer {
    processor: GroupInputProcessorRef,
    source: RequestSourceInfo,
}

impl GroupOutputTransformer {
    fn convert_common(&self, common: GroupOutputRequestCommon) -> GroupInputRequestCommon {
        let mut source = self.source.clone();
        if let Some(dec_id) = common.dec_id {
            source.set_dec(dec_id);
        }

        GroupInputRequestCommon { source }
    }

    pub fn new(
        processor: GroupInputProcessorRef,
        source: RequestSourceInfo,
    ) -> GroupOutputProcessorRef {
        let ret = Self { processor, source };
        Arc::new(Box::new(ret))
    }

    async fn push_proposal(
        &self,
        req: GroupPushProposalOutputRequest,
    ) -> BuckyResult<GroupPushProposalOutputResponse> {
        let in_req = GroupPushProposalInputRequest {
            common: self.convert_common(req.common),
            proposal: req.proposal,
        };

        let in_resp = self.processor.push_proposal(in_req).await?;

        let resp = GroupPushProposalOutputResponse {
            object: in_resp.object,
        };

        Ok(resp)
    }

    async fn start_service(
        &self,
        req: GroupStartServiceOutputRequest,
    ) -> BuckyResult<GroupStartServiceOutputResponse> {
        let in_req = GroupStartServiceInputRequest {
            group_id: req.group_id,
            rpath: req.rpath,
            common: self.convert_common(req.common),
        };

        let in_resp = self.processor.start_service(in_req).await?;

        let resp = GroupStartServiceOutputResponse {};

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl GroupOutputProcessor for GroupOutputTransformer {
    async fn start_service(
        &self,
        req: GroupStartServiceOutputRequest,
    ) -> BuckyResult<GroupStartServiceOutputResponse> {
        GroupOutputTransformer::start_service(self, req).await
    }

    async fn push_proposal(
        &self,
        req: GroupPushProposalOutputRequest,
    ) -> BuckyResult<GroupPushProposalOutputResponse> {
        GroupOutputTransformer::push_proposal(self, req).await
    }
}
