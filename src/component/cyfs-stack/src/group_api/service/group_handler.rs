use cyfs_base::*;
use cyfs_core::GroupProposal;
use cyfs_group_lib::{
    GroupPushProposalInputResponse, GroupStartServiceInputRequest, GroupStartServiceInputResponse,
};
use cyfs_lib::{NONInputRequestCommon, NONRequestorHelper, RequestorHelper};

use crate::{group::GroupInputProcessorRef, non::NONInputHttpRequest};

#[derive(Clone)]
pub(crate) struct GroupRequestHandler {
    processor: GroupInputProcessorRef,
}

impl GroupRequestHandler {
    pub fn new(processor: GroupInputProcessorRef) -> Self {
        Self { processor }
    }

    // 解析通用header字段
    fn decode_common_headers<State>(
        req: &NONInputHttpRequest<State>,
    ) -> BuckyResult<NONInputRequestCommon> {
        // req_path
        let req_path = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_REQ_PATH,
        )?;

        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取default_action字段
        let level =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_API_LEVEL)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = NONInputRequestCommon {
            req_path,
            source: req.source.clone(),
            level: level.unwrap_or_default(),
            target,
            flags: flags.unwrap_or(0),
        };

        Ok(ret)
    }

    // group/service
    pub async fn process_start_service<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> tide::Response {
        match self.on_start_service(req).await {
            Ok(_resp) => {
                let http_resp: tide::Response = RequestorHelper::new_ok_response();
                http_resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_start_service<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        let common = Self::decode_common_headers(&req)?;

        // 提取body里面的object对象，如果有的话
        let body = req.request.body_json().await.map_err(|e| {
            let msg = format!("group start service failed, read body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req = GroupStartServiceInputRequest::decode_json(&body)?;

        self.processor.start_service(common, req).await
    }

    pub async fn process_push_proposal<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> tide::Response {
        match self.on_push_proposal(req).await {
            Ok(resp) => Self::encode_push_proposal_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    pub fn encode_push_proposal_response(resp: GroupPushProposalInputResponse) -> tide::Response {
        let mut http_resp = RequestorHelper::new_response(tide::StatusCode::Ok);

        if let Some(object) = resp.object {
            NONRequestorHelper::encode_object_info(&mut http_resp, object);
        }

        http_resp.into()
    }

    async fn on_push_proposal<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        // 检查action
        // let action = Self::decode_action(&req, NONAction::PutObject)?;
        // if action != NONAction::PutObject {
        //     let msg = format!("invalid non put_object action! {:?}", action);
        //     error!("{}", msg);

        //     return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        // }

        let common = Self::decode_common_headers(&req)?;
        let object = NONRequestorHelper::decode_object_info(&mut req.request).await?;
        let (proposal, remain) = GroupProposal::raw_decode(object.object_raw.as_slice())?;
        assert_eq!(remain.len(), 0);

        // let access: Option<u32> =
        //     RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_ACCESS)?;
        // let access = access.map(|v| AccessString::new(v));

        info!("recv push proposal: {}", object.object_id);

        self.processor.push_proposal(common, proposal).await
    }
}
