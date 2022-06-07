use super::processor::*;
use super::request::*;
use super::url::*;
use cyfs_base::*;
use cyfs_lib::*;

use http_types::Request;
use std::str::FromStr;
use std::sync::Arc;
use tide::{ParamError, Response};


#[derive(Clone)]
pub(crate) struct NDNRequestHandler {
    processor: NDNInputProcessorRef,
}

impl NDNRequestHandler {
    pub fn new(processor: NDNInputProcessorRef) -> Self {
        Self { processor }
    }

    pub async fn process_put_object_request<State>(&self, req: NDNInputHttpRequest<State>) -> Response {
        let ret = self.on_put_object(req).await;
        match ret {
            Ok(_) => {
                RequestorHelper::new_ok_response()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    // 解析通用header字段
    fn decode_common_headers<State>(
        req: &NDNInputHttpRequest<State>,
    ) -> BuckyResult<NDNInputRequestCommon> {
        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取dec字段
        let dec_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_DEC_ID)?;

        // 尝试提取default_action字段
        let level =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_API_LEVEL)?;

        // 尝试提取字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = NDNInputRequestCommon {
            req_path: None,
            request: None,

            source: req.source.clone(),
            protocol: req.protocol.clone(),

            dec_id,
            leve: default_action.unwrap_or_default(),
            target,
            flags: flags.unwrap_or(0),
        };

        Ok(ret)
    }

    async fn on_put_data<State>(
        &self,
        mut req: NDNInputHttpRequest<State>,
        param: NDNPutObjectUrlParam,
    ) -> BuckyResult<NDNPutDataInputResponse> {
        let mut common = Self::decode_common_headers(&req)?;

        // 提取body
        let data = req.request.take_body().into_reader();

        common.req_path = param.req_path;
        common.request = Some(req.request.into());
        let put_req = NDNPutDataInputRequest {
            common,
            object_id: param.object_id,

            data,
        };

        self.processor.on_put_data(put_req).await
    }
}
