use crate::non::*;
use crate::rmeta::GlobalStateMetaInputProcessorRef;
use cyfs_base::*;
use cyfs_lib::*;

use http_types::StatusCode;
use tide::Response;

#[derive(Clone)]
pub(crate) struct GlobalStateMetaRequestHandler {
    processor: GlobalStateMetaInputProcessorRef,
}

impl GlobalStateMetaRequestHandler {
    pub fn new(processor: GlobalStateMetaInputProcessorRef) -> Self {
        Self { processor }
    }

    // 提取action字段
    fn decode_action<State>(
        req: &NONInputHttpRequest<State>,
        default_action: MetaAction,
    ) -> BuckyResult<MetaAction> {
        match RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_META_ACTION)? {
            Some(v) => Ok(v),
            None => Ok(default_action),
        }
    }

    // 解析通用header字段
    fn decode_common_headers<State>(
        req: &NONInputHttpRequest<State>,
    ) -> BuckyResult<MetaInputRequestCommon> {
        // 尝试提取flags
        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_FLAGS)?;

        // 尝试提取dec字段
        let target_dec_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET_DEC_ID)?;

        // 尝试提取target字段
        let target = RequestorHelper::decode_optional_header(&req.request, cyfs_base::CYFS_TARGET)?;

        let ret = MetaInputRequestCommon {
            target_dec_id,
            source: req.source.clone(),
            target,
            flags: flags.unwrap_or(0),
        };

        Ok(ret)
    }

    // add_access
    pub fn encode_add_access_response(resp: GlobalStateMetaAddAccessInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(serde_json::to_string(&resp).unwrap());
        http_resp.into()
    }

    pub async fn process_add_access_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_add_access(req).await;
        match ret {
            Ok(resp) => Self::encode_add_access_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_add_access<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GlobalStateMetaAddAccessInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, MetaAction::GlobalStateAddAccess)?;
        if action != MetaAction::GlobalStateAddAccess {
            let msg = format!("invalid global state meta add access action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let req: GlobalStateMetaAddAccessOutputRequest =
            RequestorHelper::decode_serde_json_body(&mut req.request).await?;

        let add_request = GlobalStateMetaAddAccessInputRequest {
            common,
            item: req.item,
        };

        info!(
            "recv global state meta add access request: {:?}",
            add_request
        );

        self.processor.add_access(add_request).await
    }

    // remove_access
    pub fn encode_remove_access_response(
        resp: GlobalStateMetaRemoveAccessInputResponse,
    ) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(serde_json::to_string(&resp).unwrap());
        http_resp.into()
    }

    pub async fn process_remove_access_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_remove_access(req).await;
        match ret {
            Ok(resp) => Self::encode_remove_access_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_remove_access<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, MetaAction::GlobalStateRemoveAccess)?;
        if action != MetaAction::GlobalStateRemoveAccess {
            let msg = format!(
                "invalid global state meta remove access action! {:?}",
                action
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let req: GlobalStateMetaAddAccessOutputRequest =
            RequestorHelper::decode_serde_json_body(&mut req.request).await?;

        let add_request = GlobalStateMetaRemoveAccessInputRequest {
            common,
            item: req.item,
        };

        info!(
            "recv global state meta remove access request: {:?}",
            add_request
        );

        self.processor.remove_access(add_request).await
    }

    // clear_access
    pub fn encode_clear_access_response(resp: GlobalStateMetaClearAccessInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(serde_json::to_string(&resp).unwrap());
        http_resp.into()
    }

    pub async fn process_clear_access_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_clear_access(req).await;
        match ret {
            Ok(resp) => Self::encode_clear_access_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_clear_access<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GlobalStateMetaClearAccessInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, MetaAction::GlobalStateClearAccess)?;
        if action != MetaAction::GlobalStateClearAccess {
            let msg = format!(
                "invalid global state meta clear access action! {:?}",
                action
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;
        let clear_request = GlobalStateMetaClearAccessInputRequest { common };

        info!(
            "recv global state meta clear access request: {:?}",
            clear_request
        );

        self.processor.clear_access(clear_request).await
    }

    // add_link
    pub fn encode_add_link_response(resp: GlobalStateMetaAddLinkInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(serde_json::to_string(&resp).unwrap());
        http_resp.into()
    }

    pub async fn process_add_link_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_add_link(req).await;
        match ret {
            Ok(resp) => Self::encode_add_link_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_add_link<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GlobalStateMetaAddLinkInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, MetaAction::GlobalStateAddLink)?;
        if action != MetaAction::GlobalStateAddLink {
            let msg = format!("invalid global state meta add link action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let req: GlobalStateMetaAddLinkOutputRequest =
            RequestorHelper::decode_serde_json_body(&mut req.request).await?;

        let add_request = GlobalStateMetaAddLinkInputRequest {
            common,
            source: req.source,
            target: req.target,
        };

        info!("recv global state meta add link request: {:?}", add_request);

        self.processor.add_link(add_request).await
    }

    // remove_link
    pub fn encode_remove_link_response(resp: GlobalStateMetaRemoveLinkInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(serde_json::to_string(&resp).unwrap());
        http_resp.into()
    }

    pub async fn process_remove_link_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_remove_link(req).await;
        match ret {
            Ok(resp) => Self::encode_remove_link_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_remove_link<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, MetaAction::GlobalStateRemoveLink)?;
        if action != MetaAction::GlobalStateRemoveLink {
            let msg = format!("invalid global state meta remove link action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let req: GlobalStateMetaRemoveLinkOutputRequest =
            RequestorHelper::decode_serde_json_body(&mut req.request).await?;

        let add_request = GlobalStateMetaRemoveLinkInputRequest {
            common,
            source: req.source,
        };

        info!(
            "recv global state meta remove link request: {:?}",
            add_request
        );

        self.processor.remove_link(add_request).await
    }

    // clear_link
    pub fn encode_clear_link_response(resp: GlobalStateMetaClearLinkInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(serde_json::to_string(&resp).unwrap());
        http_resp.into()
    }

    pub async fn process_clear_link_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_clear_link(req).await;
        match ret {
            Ok(resp) => Self::encode_clear_link_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_clear_link<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GlobalStateMetaClearLinkInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, MetaAction::GlobalStateClearLink)?;
        if action != MetaAction::GlobalStateClearLink {
            let msg = format!("invalid global state meta clear link action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;
        let clear_request = GlobalStateMetaClearLinkInputRequest { common };

        info!(
            "recv global state meta clear link request: {:?}",
            clear_request
        );

        self.processor.clear_link(clear_request).await
    }

    // add_object_meta
    pub fn encode_add_object_meta_response(resp: GlobalStateMetaAddObjectMetaInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(serde_json::to_string(&resp).unwrap());
        http_resp.into()
    }

    pub async fn process_add_object_meta_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_add_object_meta(req).await;
        match ret {
            Ok(resp) => Self::encode_add_object_meta_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_add_object_meta<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GlobalStateMetaAddObjectMetaInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, MetaAction::GlobalStateAddObjectMeta)?;
        if action != MetaAction::GlobalStateAddObjectMeta {
            let msg = format!("invalid global state meta add object meta action! {:?}", action);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let req: GlobalStateMetaAddObjectMetaOutputRequest =
            RequestorHelper::decode_serde_json_body(&mut req.request).await?;

        let add_request = GlobalStateMetaAddObjectMetaInputRequest {
            common,
            item: req.item,
        };

        info!(
            "recv global state meta add object meta request: {:?}",
            add_request
        );

        self.processor.add_object_meta(add_request).await
    }

    // remove_object_meta
    pub fn encode_remove_object_meta_response(
        resp: GlobalStateMetaRemoveObjectMetaInputResponse,
    ) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(serde_json::to_string(&resp).unwrap());
        http_resp.into()
    }

    pub async fn process_remove_object_meta_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_remove_object_meta(req).await;
        match ret {
            Ok(resp) => Self::encode_remove_object_meta_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_remove_object_meta<State: Send>(
        &self,
        mut req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GlobalStateMetaRemoveObjectMetaInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, MetaAction::GlobalStateRemoveObjectMeta)?;
        if action != MetaAction::GlobalStateRemoveObjectMeta {
            let msg = format!(
                "invalid global state meta remove object meta action! {:?}",
                action
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;

        let req: GlobalStateMetaAddObjectMetaOutputRequest =
            RequestorHelper::decode_serde_json_body(&mut req.request).await?;

        let add_request = GlobalStateMetaRemoveObjectMetaInputRequest {
            common,
            item: req.item,
        };

        info!(
            "recv global state meta remove object meta request: {:?}",
            add_request
        );

        self.processor.remove_object_meta(add_request).await
    }

    // clear_object_meta
    pub fn encode_clear_object_meta_response(resp: GlobalStateMetaClearObjectMetaInputResponse) -> Response {
        let mut http_resp = RequestorHelper::new_response(StatusCode::Ok);

        http_resp.set_body(serde_json::to_string(&resp).unwrap());
        http_resp.into()
    }

    pub async fn process_clear_object_meta_request<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> Response {
        let ret = self.on_clear_object_meta(req).await;
        match ret {
            Ok(resp) => Self::encode_clear_object_meta_response(resp),
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_clear_object_meta<State: Send>(
        &self,
        req: NONInputHttpRequest<State>,
    ) -> BuckyResult<GlobalStateMetaClearObjectMetaInputResponse> {
        // 检查action
        let action = Self::decode_action(&req, MetaAction::GlobalStateClearObjectMeta)?;
        if action != MetaAction::GlobalStateClearObjectMeta {
            let msg = format!(
                "invalid global state meta clear object meta action! {:?}",
                action
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let common = Self::decode_common_headers(&req)?;
        let clear_request = GlobalStateMetaClearObjectMetaInputRequest { common };

        info!(
            "recv global state meta clear object meta request: {:?}",
            clear_request
        );

        self.processor.clear_object_meta(clear_request).await
    }
}
