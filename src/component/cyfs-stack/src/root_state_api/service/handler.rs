use crate::front::FrontRequestObjectFormat;
use crate::non_api::NONRequestHandler;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_lib::*;

use http_types::StatusCode;
use std::str::FromStr;
use tide::Response;

#[derive(Clone)]
pub(crate) struct GlobalStateRequestHandler {
    processor: GlobalStateInputProcessorRef,
}

impl GlobalStateRequestHandler {
    pub fn new(processor: GlobalStateInputProcessorRef) -> Self {
        Self { processor }
    }

    // 提取action字段
    fn decode_action<State>(
        req: &RootStateInputHttpRequest<State>,
    ) -> BuckyResult<RootStateAction> {
        RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_ROOT_STATE_ACTION)
    }

    // 解析通用header字段
    fn decode_common_headers<State>(
        req: &RootStateInputHttpRequest<State>,
    ) -> BuckyResult<RootStateInputRequestCommon> {
        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取dec字段
        let dec_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_DEC_ID)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = RootStateInputRequestCommon {
            source: req.source.clone(),
            protocol: req.protocol.clone(),

            dec_id,
            target,

            flags: flags.unwrap_or(0),
        };

        Ok(ret)
    }

    pub async fn process_get_current_root_request<State: Send>(
        &self,
        req: RootStateInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_current_root(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_current_root<State: Send>(
        &self,
        mut req: RootStateInputHttpRequest<State>,
    ) -> BuckyResult<RootStateGetCurrentRootInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != RootStateAction::GetCurrentRoot {
            let msg = format!("invalid root state get_current_root action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let output_req: RootStateGetCurrentRootOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let common = Self::decode_common_headers(&req)?;

        let req = RootStateGetCurrentRootInputRequest {
            common,
            root_type: output_req.root_type,
        };

        info!("recv get_current_root request: {}", req);

        self.processor.get_current_root(req).await
    }

    pub async fn process_create_op_env_request<State: Send>(
        &self,
        req: RootStateInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_create_op_env(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_create_op_env<State: Send>(
        &self,
        mut req: RootStateInputHttpRequest<State>,
    ) -> BuckyResult<RootStateCreateOpEnvInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != RootStateAction::CreateOpEnv {
            let msg = format!("invalid root state create_op_env action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let output_req: RootStateCreateOpEnvOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let common = Self::decode_common_headers(&req)?;

        let req = RootStateCreateOpEnvInputRequest {
            common,
            op_env_type: output_req.op_env_type,
        };

        info!("recv create_op_env request: {}", req);

        self.processor.create_op_env(req).await
    }
}

#[derive(Clone)]
pub(crate) struct OpEnvRequestHandler {
    processor: OpEnvInputProcessorRef,
}

impl OpEnvRequestHandler {
    pub fn new(processor: OpEnvInputProcessorRef) -> Self {
        Self { processor }
    }

    // 提取action字段
    fn decode_action<State>(req: &OpEnvInputHttpRequest<State>) -> BuckyResult<OpEnvAction> {
        RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_OP_ENV_ACTION)
    }

    // 解析通用header字段
    fn decode_common_headers<State>(
        req: &OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvInputRequestCommon> {
        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取dec字段
        let dec_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_DEC_ID)?;

        // 提取sid
        let sid: u64 = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_OP_ENV_SID)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = OpEnvInputRequestCommon {
            source: req.source.clone(),
            protocol: req.protocol.clone(),

            dec_id,
            target,

            flags: flags.unwrap_or(0),
            sid,
        };

        Ok(ret)
    }

    // load
    pub async fn process_load_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_load(req).await;
        match ret {
            Ok(_) => {
                let http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_load<State: Send>(&self, mut req: OpEnvInputHttpRequest<State>) -> BuckyResult<()> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Load {
            let msg = format!("invalid op_env load action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvLoadOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvLoadInputRequest {
            common,
            target: output_req.target,
        };

        info!("recv op_env load request: {}", req);

        self.processor.load(req).await
    }

    // load_by_path
    pub async fn process_load_by_path_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_load_by_path(req).await;
        match ret {
            Ok(_) => {
                let http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_load_by_path<State: Send>(
        &self,
        mut req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<()> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::LoadByPath {
            let msg = format!("invalid op_env load_by_path action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvLoadByPathOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvLoadByPathInputRequest {
            common,
            path: output_req.path,
        };

        info!("recv op_env load_by_path request: {}", req);

        self.processor.load_by_path(req).await
    }

    // create_new
    pub async fn process_create_new_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_create_new(req).await;
        match ret {
            Ok(_) => {
                let http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_create_new<State: Send>(
        &self,
        mut req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<()> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::CreateNew {
            let msg = format!("invalid op_env create_new action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvCreateNewOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvCreateNewInputRequest {
            common,
            path: output_req.path,
            key: output_req.key,
            content_type: output_req.content_type,
        };

        info!("recv op_env create_new request: {}", req);

        self.processor.create_new(req).await
    }

    // lock
    pub async fn process_lock_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_lock(req).await;
        match ret {
            Ok(_) => {
                let http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_lock<State: Send>(&self, mut req: OpEnvInputHttpRequest<State>) -> BuckyResult<()> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Lock {
            let msg = format!("invalid op_env lock action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvLockOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvLockInputRequest {
            common,
            path_list: output_req.path_list,
            duration_in_millsecs: output_req.duration_in_millsecs,
            try_lock: output_req.try_lock,
        };

        info!("recv op_env lock request: {}", req);

        self.processor.lock(req).await
    }

    // get_current_root
    pub async fn process_get_current_root_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_current_root(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_current_root<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvGetCurrentRootInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::GetCurrentRoot {
            let msg = format!("invalid op_env get_current_root action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let req = OpEnvGetCurrentRootInputRequest { common };

        info!("recv op_env get_current_root request: {}", req);

        self.processor.get_current_root(req).await
    }

    // commit
    pub async fn process_commit_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_commit(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_commit<State: Send>(
        &self,
        mut req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvCommitInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Commit {
            let msg = format!("invalid op_env commit action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvCommitOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;

        let req = OpEnvCommitInputRequest { common, op_type: output_req.op_type };

        info!("recv op_env commit request: {}", req);

        self.processor.commit(req).await
    }

    // abort
    pub async fn process_abort_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_abort(req).await;
        match ret {
            Ok(_) => {
                let http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_abort<State: Send>(&self, req: OpEnvInputHttpRequest<State>) -> BuckyResult<()> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Abort {
            let msg = format!("invalid op_env abort action! {:?}", action);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }
        let common = Self::decode_common_headers(&req)?;

        let req = OpEnvAbortInputRequest { common };
        info!("recv op_env abort request: {}", req);
        self.processor.abort(req).await
    }

    // metadata
    pub async fn process_metadata_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_metadata(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_metadata<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvMetadataInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Metadata {
            let msg = format!("invalid op_env metadata action! {:?}", action);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }
        let common = Self::decode_common_headers(&req)?;

        let path = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_OP_ENV_PATH,
        )?;

        let req = OpEnvMetadataInputRequest { common, path };
        info!("recv op_env metadata request: {}", req);
        self.processor.metadata(req).await
    }

    // get_by_key
    pub async fn process_get_by_key_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_get_by_key(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_get_by_key<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvGetByKeyInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::GetByKey {
            let msg = format!("invalid op_env get_by_key action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let path = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_OP_ENV_PATH,
        )?;
        let key = RequestorHelper::decode_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_OP_ENV_KEY,
        )?;

        let req = OpEnvGetByKeyInputRequest { common, path, key };

        info!("recv op_env get_by_key request: {}", req);

        self.processor.get_by_key(req).await
    }

    // insert_with_key
    pub async fn process_insert_with_key_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_insert_with_key(req).await;
        match ret {
            Ok(_) => {
                let http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_insert_with_key<State: Send>(
        &self,
        mut req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<()> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::InsertWithKey {
            let msg = format!("invalid op_env insert_with_key action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvInsertWithKeyOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvInsertWithKeyInputRequest {
            common,
            path: output_req.path,
            key: output_req.key,
            value: output_req.value,
        };

        info!("recv op_env insert_with_key request: {}", req);

        self.processor.insert_with_key(req).await
    }

    // set_with_key
    pub async fn process_set_with_key_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_set_with_key(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_set_with_key<State: Send>(
        &self,
        mut req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvSetWithKeyInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::SetWithKey {
            let msg = format!("invalid op_env set_with_key action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvSetWithKeyOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvSetWithKeyInputRequest {
            common,
            path: output_req.path,
            key: output_req.key,
            value: output_req.value,
            prev_value: output_req.prev_value,
            auto_insert: output_req.auto_insert,
        };

        info!("recv op_env set_with_key request: {}", req);

        self.processor.set_with_key(req).await
    }

    // remove_with_key
    pub async fn process_remove_with_key_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_remove_with_key(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_remove_with_key<State: Send>(
        &self,
        mut req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvRemoveWithKeyInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::RemoveWithKey {
            let msg = format!("invalid op_env remove_with_key action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvRemoveWithKeyOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvRemoveWithKeyInputRequest {
            common,
            path: output_req.path,
            key: output_req.key,
            prev_value: output_req.prev_value,
        };

        info!("recv op_env remove_with_key request: {}", req);

        self.processor.remove_with_key(req).await
    }

    // contains
    pub async fn process_contains_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_contains(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_contains<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvContainsOutputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Contains {
            let msg = format!("invalid op_env contains action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let path = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_OP_ENV_PATH,
        )?;
        let value = RequestorHelper::decode_header(&req.request, cyfs_base::CYFS_OP_ENV_VALUE)?;

        let req = OpEnvContainsInputRequest {
            common,
            path,
            value,
        };

        info!("recv op_env contains request: {}", req);

        self.processor.contains(req).await
    }

    // insert
    pub async fn process_insert_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_insert(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_insert<State: Send>(
        &self,
        mut req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvInsertInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Insert {
            let msg = format!("invalid op_env insert action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvInsertOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvInsertInputRequest {
            common,
            path: output_req.path,
            value: output_req.value,
        };

        info!("recv op_env insert request: {}", req);

        self.processor.insert(req).await
    }

    // remove
    pub async fn process_remove_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_remove(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_remove<State: Send>(
        &self,
        mut req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvRemoveInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Remove {
            let msg = format!("invalid op_env remove action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvRemoveOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvRemoveInputRequest {
            common,
            path: output_req.path,
            value: output_req.value,
        };

        info!("recv op_env remove request: {}", req);

        self.processor.remove(req).await
    }

    // next
    pub async fn process_next_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_next(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_next<State: Send>(
        &self,
        mut req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvNextInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Next {
            let msg = format!("invalid op_env next action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let output_req: OpEnvNextOutputRequest =
            RequestorHelper::decode_json_body(&mut req.request).await?;
        let req = OpEnvNextInputRequest {
            common,
            step: output_req.step,
        };

        debug!("recv op_env next request: {}", req);

        self.processor.next(req).await
    }

    // reset
    pub async fn process_reset_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_reset(req).await;
        match ret {
            Ok(_) => {
                let http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_reset<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<()> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::Reset {
            let msg = format!("invalid op_env reset action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;


        let req = OpEnvResetInputRequest {
            common,
        };

        debug!("recv op_env reset request: {}", req);

        self.processor.reset(req).await
    }

    // next
    pub async fn process_list_request<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_list(req).await;
        match ret {
            Ok(resp) => {
                let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);
                http_resp.set_body(resp.encode_string());
                http_resp.into()
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_list<State: Send>(
        &self,
        req: OpEnvInputHttpRequest<State>,
    ) -> BuckyResult<OpEnvListInputResponse> {
        // 检查action
        let action = Self::decode_action(&req)?;
        if action != OpEnvAction::List {
            let msg = format!("invalid op_env list action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;
        let path = RequestorHelper::decode_optional_header_with_utf8_decoding(
            &req.request,
            cyfs_base::CYFS_OP_ENV_PATH,
        )?;

        let req = OpEnvListInputRequest {
            common,
            path,
        };

        debug!("recv op_env list request: {}", req);

        self.processor.list(req).await
    }
}

#[derive(Clone)]
pub(crate) struct GlobalStateAccessRequestHandler {
    processor: GlobalStateAccessInputProcessorRef,
}

impl GlobalStateAccessRequestHandler {
    pub fn new(processor: GlobalStateAccessInputProcessorRef) -> Self {
        Self { processor }
    }

    // 解析通用header字段
    fn decode_common_headers<State>(
        req: &RootStateInputHttpRequest<State>,
    ) -> BuckyResult<RootStateInputRequestCommon> {
        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取dec字段
        let dec_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_DEC_ID)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = RootStateInputRequestCommon {
            source: req.source.clone(),
            protocol: req.protocol.clone(),

            dec_id,
            target,

            flags: flags.unwrap_or(0),
        };

        Ok(ret)
    }

    pub async fn process_access_request<State: Send>(
        &self,
        req: RootStateInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_access(req).await;
        match ret {
            Ok(resp) => resp,
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_access<State: Send>(
        &self,
        req: RootStateInputHttpRequest<State>,
    ) -> BuckyResult<Response> {
        // extract params from url querys
        let mut page_index: Option<u32> = None;
        let mut page_size: Option<u32> = None;
        let mut action = RootStateAccessAction::GetObjectByPath;

        let pairs = req.request.url().query_pairs();
        for (k, v) in pairs {
            match k.as_ref() {
                "action" => {
                    action = RootStateAccessAction::from_str(v.as_ref())?;
                }
                "page_index" => {
                    let v = v.as_ref().parse().map_err(|e| {
                        let msg = format!("invalid page_index param: {}, {}", v, e);
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::InvalidParam, msg)
                    })?;
                    page_index = Some(v);
                }
                "page_size" => {
                    let v = v.as_ref().parse().map_err(|e| {
                        let msg = format!("invalid page_size param: {}, {}", v, e);
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::InvalidParam, msg)
                    })?;
                    page_size = Some(v);
                }
                _ => {
                    warn!("unknown global state access url query: {}={}", k, v);
                }
            }
        }

        let inner_path = req.request.param("inner_path").unwrap_or("/");
        let inner_path = RequestorHelper::decode_utf8("inner_path", inner_path)?;

        let inner_path = if inner_path.starts_with("/") {
            inner_path
        } else {
            format!("/{}", inner_path)
        };

        let common = Self::decode_common_headers(&req)?;

        match action {
            RootStateAccessAction::GetObjectByPath => {
                let req = RootStateAccessGetObjectByPathInputRequest { common, inner_path };
                self.on_get_object_by_path(req).await
            }
            RootStateAccessAction::List => {
                let req = RootStateAccessListInputRequest {
                    common,
                    inner_path,
                    page_index,
                    page_size,
                };
                self.on_list(req).await
            }
        }
    }

    fn encode_get_object_by_path_response(
        resp: RootStateAccessGetObjectByPathInputResponse,
    ) -> Response {
        let mut http_resp = NONRequestHandler::encode_get_object_response(
            resp.object,
            FrontRequestObjectFormat::Raw,
        );
        http_resp.insert_header(cyfs_base::CYFS_ROOT, resp.root.to_string());
        http_resp.insert_header(cyfs_base::CYFS_REVISION, resp.revision.to_string());

        http_resp
    }

    async fn on_get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathInputRequest,
    ) -> BuckyResult<Response> {
        let resp = self.processor.get_object_by_path(req).await?;

        let http_resp = Self::encode_get_object_by_path_response(resp);
        Ok(http_resp)
    }

    async fn on_list(&self, req: RootStateAccessListInputRequest) -> BuckyResult<Response> {
        let resp = self.processor.list(req).await?;

        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(resp.list.encode_string());

        http_resp.set_content_type(tide::http::mime::JSON);
        http_resp.insert_header(cyfs_base::CYFS_ROOT, resp.root.to_string());
        http_resp.insert_header(cyfs_base::CYFS_REVISION, resp.revision.to_string());

        Ok(http_resp.into())
    }
}
