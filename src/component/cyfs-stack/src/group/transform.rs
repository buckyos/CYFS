use cyfs_base::*;
use cyfs_core::GroupProposal;
use cyfs_group_lib::{
    GroupOutputProcessor, GroupOutputProcessorRef, GroupPushProposalInputResponse,
    GroupPushProposalOutputResponse, GroupStartServiceInputRequest, GroupStartServiceInputResponse,
    GroupStartServiceOutputRequest, GroupStartServiceOutputResponse,
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

    fn convert_common(common: NONInputRequestCommon) -> NONOutputRequestCommon {
        NONOutputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 来源DEC
            dec_id: common.source.get_opt_dec().cloned(),

            // 默认行为
            level: common.level,

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,

            source: common.source.zone.device,
        }
    }

    async fn start_service(
        &self,
        req_common: NONInputRequestCommon,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        let out_req = GroupStartServiceOutputRequest {
            group_id: req.group_id,
            rpath: req.rpath,
        };

        let out_resp = self
            .processor
            .start_service(Self::convert_common(req_common), out_req)
            .await?;

        let resp = GroupStartServiceInputResponse {};

        Ok(resp)
    }

    async fn push_proposal(
        &self,
        req_common: NONInputRequestCommon,
        req: GroupProposal,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        let out_resp = self
            .processor
            .push_proposal(Self::convert_common(req_common), req)
            .await?;

        let resp = GroupPushProposalInputResponse {};

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl GroupInputProcessor for GroupInputTransformer {
    async fn start_service(
        &self,
        req_common: NONInputRequestCommon,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        GroupInputTransformer::start_service(self, req_common, req).await
    }

    async fn push_proposal(
        &self,
        req_common: NONInputRequestCommon,
        req: GroupProposal,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        GroupInputTransformer::push_proposal(self, req_common, req).await
    }
}

// 实现从output到input的转换
pub(crate) struct GroupOutputTransformer {
    processor: GroupInputProcessorRef,
    source: RequestSourceInfo,
}

impl GroupOutputTransformer {
    fn convert_common(&self, common: NONOutputRequestCommon) -> NONInputRequestCommon {
        let mut source = self.source.clone();
        if let Some(dec_id) = common.dec_id {
            source.set_dec(dec_id);
        }

        NONInputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 默认行为
            level: common.level,

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,

            source,
        }
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
        req_common: NONOutputRequestCommon,
        req: GroupProposal,
    ) -> BuckyResult<GroupPushProposalOutputResponse> {
        let in_resp = self
            .processor
            .push_proposal(self.convert_common(req_common), req)
            .await?;

        let resp = GroupPushProposalOutputResponse {};

        Ok(resp)
    }

    async fn start_service(
        &self,
        req_common: NONOutputRequestCommon,
        req: GroupStartServiceOutputRequest,
    ) -> BuckyResult<GroupStartServiceOutputResponse> {
        let in_req = GroupStartServiceInputRequest {
            group_id: req.group_id,
            rpath: req.rpath,
        };

        let in_resp = self
            .processor
            .start_service(self.convert_common(req_common), in_req)
            .await?;

        let resp = GroupStartServiceOutputResponse {};

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl GroupOutputProcessor for GroupOutputTransformer {
    async fn start_service(
        &self,
        req_common: NONOutputRequestCommon,
        req: GroupStartServiceOutputRequest,
    ) -> BuckyResult<GroupStartServiceOutputResponse> {
        GroupOutputTransformer::start_service(self, req_common, req).await
    }

    async fn push_proposal(
        &self,
        req_common: NONOutputRequestCommon,
        req: GroupProposal,
    ) -> BuckyResult<GroupPushProposalOutputResponse> {
        GroupOutputTransformer::push_proposal(self, req_common, req).await
    }
}
