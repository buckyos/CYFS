use super::def::*;
use super::output_request::*;
use super::processor::*;
use crate::base::*;
use crate::root_state::*;
use crate::stack::SharedObjectStackDecID;
use cyfs_base::*;

use http_types::{Method, Request, Url};
use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalStateMetaRequestor {
    category: GlobalStateCategory,
    dec_id: Option<SharedObjectStackDecID>,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl GlobalStateMetaRequestor {
    pub fn new_default_tcp(
        category: GlobalStateCategory,
        dec_id: Option<SharedObjectStackDecID>,
    ) -> Self {
        let service_addr = format!("127.0.0.1:{}", cyfs_base::NON_STACK_HTTP_PORT);
        Self::new_tcp(category, dec_id, &service_addr)
    }

    pub fn new_tcp(
        category: GlobalStateCategory,
        dec_id: Option<SharedObjectStackDecID>,
        service_addr: &str,
    ) -> Self {
        let tcp_requestor = TcpHttpRequestor::new(service_addr);
        Self::new(category, dec_id, Arc::new(Box::new(tcp_requestor)))
    }

    pub fn new_root_state(
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        Self::new(GlobalStateCategory::RootState, dec_id, requestor)
    }

    pub fn new_local_cache(
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        Self::new(GlobalStateCategory::LocalCache, dec_id, requestor)
    }

    pub fn new(
        category: GlobalStateCategory,
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/{}/meta/", addr, category.as_str());
        let url = Url::parse(&url).unwrap();

        let ret = Self {
            category,
            dec_id,
            requestor,
            service_url: url,
        };

        ret
    }

    pub fn category(&self) -> &GlobalStateCategory {
        &self.category
    }

    pub fn into_processor(self) -> GlobalStateMetaOutputProcessorRef {
        Arc::new(Box::new(self))
    }

    pub fn clone_processor(&self) -> GlobalStateMetaOutputProcessorRef {
        self.clone().into_processor()
    }

    // TODO: 目前和request的body部分编码一部分冗余的信息
    fn encode_common_headers(
        &self,
        action: MetaAction,
        com_req: &MetaOutputRequestCommon,
        http_req: &mut Request,
    ) {
        if let Some(dec_id) = &com_req.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        } else if let Some(dec_id) = &self.dec_id {
            if let Some(dec_id) = dec_id.get() {
                http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
            }
        }

        if let Some(target_dec_id) = &com_req.target_dec_id {
            http_req.insert_header(cyfs_base::CYFS_TARGET_DEC_ID, target_dec_id.to_string());
        }

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());

        http_req.insert_header(cyfs_base::CYFS_META_ACTION, action.to_string());
    }

    // global-state-meta add-access
    fn encode_add_access_request(&self, req: &GlobalStateMetaAddAccessOutputRequest) -> Request {
        let url = self.service_url.join("access").unwrap();
        let mut http_req = Request::new(Method::Put, url);
        self.encode_common_headers(MetaAction::GlobalStateAddAccess, &req.common, &mut http_req);

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessOutputResponse> {
        let http_req = self.encode_add_access_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: GlobalStateMetaAddAccessOutputResponse =
                RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta add access success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("global state meta add access error! req={:?}, {}", req, e);
            Err(e)
        }
    }

    // global-state-meta remove-access
    fn encode_remove_access_request(
        &self,
        req: &GlobalStateMetaRemoveAccessOutputRequest,
    ) -> Request {
        let url = self.service_url.join("access").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(
            MetaAction::GlobalStateRemoveAccess,
            &req.common,
            &mut http_req,
        );

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessOutputResponse> {
        let http_req = self.encode_remove_access_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: GlobalStateMetaRemoveAccessOutputResponse =
                RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta remove access success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "global state meta remove access error! req={:?}, {}",
                req, e
            );
            Err(e)
        }
    }

    // global-state-meta clear-access
    fn encode_clear_access_request(
        &self,
        req: &GlobalStateMetaClearAccessOutputRequest,
    ) -> Request {
        let url = self.service_url.join("accesses").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(
            MetaAction::GlobalStateClearAccess,
            &req.common,
            &mut http_req,
        );

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessOutputResponse> {
        let http_req = self.encode_clear_access_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: GlobalStateMetaClearAccessOutputResponse =
                RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta clear access success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("global state meta clear access error! req={:?}, {}", req, e);
            Err(e)
        }
    }

    // global-state-meta add-link
    fn encode_add_link_request(&self, req: &GlobalStateMetaAddLinkOutputRequest) -> Request {
        let url = self.service_url.join("link").unwrap();
        let mut http_req = Request::new(Method::Put, url);
        self.encode_common_headers(MetaAction::GlobalStateAddLink, &req.common, &mut http_req);

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkOutputResponse> {
        let http_req = self.encode_add_link_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta add link success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("global state meta add link error! req={:?}, {}", req, e);
            Err(e)
        }
    }

    // global-state-meta remove-access
    fn encode_remove_link_request(&self, req: &GlobalStateMetaRemoveLinkOutputRequest) -> Request {
        let url = self.service_url.join("link").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(
            MetaAction::GlobalStateRemoveLink,
            &req.common,
            &mut http_req,
        );

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkOutputResponse> {
        let http_req = self.encode_remove_link_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta remove link success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("global state meta remove link error! req={:?}, {}", req, e);
            Err(e)
        }
    }

    // global-state-meta clear-link
    fn encode_clear_link_request(&self, req: &GlobalStateMetaClearLinkOutputRequest) -> Request {
        let url = self.service_url.join("links").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(MetaAction::GlobalStateClearLink, &req.common, &mut http_req);

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkOutputResponse> {
        let http_req = self.encode_clear_link_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta clear links success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("global state meta clear links error! req={:?}, {}", req, e);
            Err(e)
        }
    }

    // global-state-meta add-object_meta
    fn encode_add_object_meta_request(
        &self,
        req: &GlobalStateMetaAddObjectMetaOutputRequest,
    ) -> Request {
        let url = self.service_url.join("object-meta").unwrap();
        let mut http_req = Request::new(Method::Put, url);
        self.encode_common_headers(
            MetaAction::GlobalStateAddObjectMeta,
            &req.common,
            &mut http_req,
        );

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn add_object_meta(
        &self,
        req: GlobalStateMetaAddObjectMetaOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddObjectMetaOutputResponse> {
        let http_req = self.encode_add_object_meta_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: GlobalStateMetaAddObjectMetaOutputResponse =
                RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta add object meta success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "global state meta add object meta error! req={:?}, {}",
                req, e
            );
            Err(e)
        }
    }

    // global-state-meta remove-object_meta
    fn encode_remove_object_meta_request(
        &self,
        req: &GlobalStateMetaRemoveObjectMetaOutputRequest,
    ) -> Request {
        let url = self.service_url.join("object-meta").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(
            MetaAction::GlobalStateRemoveObjectMeta,
            &req.common,
            &mut http_req,
        );

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn remove_object_meta(
        &self,
        req: GlobalStateMetaRemoveObjectMetaOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveObjectMetaOutputResponse> {
        let http_req = self.encode_remove_object_meta_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: GlobalStateMetaRemoveObjectMetaOutputResponse =
                RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta remove object meta success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "global state meta remove object meta error! req={:?}, {}",
                req, e
            );
            Err(e)
        }
    }

    // global-state-meta clear-object_meta
    fn encode_clear_object_meta_request(
        &self,
        req: &GlobalStateMetaClearObjectMetaOutputRequest,
    ) -> Request {
        let url = self.service_url.join("object-metas").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(
            MetaAction::GlobalStateClearObjectMeta,
            &req.common,
            &mut http_req,
        );

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn clear_object_meta(
        &self,
        req: GlobalStateMetaClearObjectMetaOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearObjectMetaOutputResponse> {
        let http_req = self.encode_clear_object_meta_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: GlobalStateMetaClearObjectMetaOutputResponse =
                RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta clear object meta success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "global state meta clear object meta error! req={:?}, {}",
                req, e
            );
            Err(e)
        }
    }

    // path config
    // global-state-meta add-path-config
    fn encode_add_path_config_request(
        &self,
        req: &GlobalStateMetaAddPathConfigOutputRequest,
    ) -> Request {
        let url = self.service_url.join("path-config").unwrap();
        let mut http_req = Request::new(Method::Put, url);
        self.encode_common_headers(
            MetaAction::GlobalStateAddPathConfig,
            &req.common,
            &mut http_req,
        );

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn add_path_config(
        &self,
        req: GlobalStateMetaAddPathConfigOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddPathConfigOutputResponse> {
        let http_req = self.encode_add_path_config_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: GlobalStateMetaAddPathConfigOutputResponse =
                RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta add path config success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "global state meta add path config error! req={:?}, {}",
                req, e
            );
            Err(e)
        }
    }

    // global-state-meta remove-path_config
    fn encode_remove_path_config_request(
        &self,
        req: &GlobalStateMetaRemovePathConfigOutputRequest,
    ) -> Request {
        let url = self.service_url.join("path-config").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(
            MetaAction::GlobalStateRemovePathConfig,
            &req.common,
            &mut http_req,
        );

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn remove_path_config(
        &self,
        req: GlobalStateMetaRemovePathConfigOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemovePathConfigOutputResponse> {
        let http_req = self.encode_remove_path_config_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: GlobalStateMetaRemovePathConfigOutputResponse =
                RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta remove path config success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "global state meta remove path config error! req={:?}, {}",
                req, e
            );
            Err(e)
        }
    }

    // global-state-meta clear-path_config
    fn encode_clear_path_config_request(
        &self,
        req: &GlobalStateMetaClearPathConfigOutputRequest,
    ) -> Request {
        let url = self.service_url.join("path-configs").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(
            MetaAction::GlobalStateClearPathConfig,
            &req.common,
            &mut http_req,
        );

        let value = serde_json::to_string(&req).unwrap();
        http_req.set_body(value);
        http_req
    }

    async fn clear_path_config(
        &self,
        req: GlobalStateMetaClearPathConfigOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearPathConfigOutputResponse> {
        let http_req = self.encode_clear_path_config_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: GlobalStateMetaClearPathConfigOutputResponse =
                RequestorHelper::decode_serde_json_body(&mut resp).await?;
            info!(
                "global state meta clear path config success: req={:?}, resp={:?}",
                req, resp,
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "global state meta clear path config error! req={:?}, {}",
                req, e
            );
            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateMetaOutputProcessor for GlobalStateMetaRequestor {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessOutputResponse> {
        Self::add_access(&self, req).await
    }

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessOutputResponse> {
        Self::remove_access(&self, req).await
    }

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessOutputResponse> {
        Self::clear_access(&self, req).await
    }

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkOutputResponse> {
        Self::add_link(&self, req).await
    }

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkOutputResponse> {
        Self::remove_link(&self, req).await
    }

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkOutputResponse> {
        Self::clear_link(&self, req).await
    }

    // object meta
    async fn add_object_meta(
        &self,
        req: GlobalStateMetaAddObjectMetaOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddObjectMetaOutputResponse> {
        Self::add_object_meta(&self, req).await
    }

    async fn remove_object_meta(
        &self,
        req: GlobalStateMetaRemoveObjectMetaOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveObjectMetaOutputResponse> {
        Self::remove_object_meta(&self, req).await
    }

    async fn clear_object_meta(
        &self,
        req: GlobalStateMetaClearObjectMetaOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearObjectMetaOutputResponse> {
        Self::clear_object_meta(&self, req).await
    }

    // path config
    async fn add_path_config(
        &self,
        req: GlobalStateMetaAddPathConfigOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddPathConfigOutputResponse> {
        Self::add_path_config(&self, req).await
    }

    async fn remove_path_config(
        &self,
        req: GlobalStateMetaRemovePathConfigOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemovePathConfigOutputResponse> {
        Self::remove_path_config(&self, req).await
    }

    async fn clear_path_config(
        &self,
        req: GlobalStateMetaClearPathConfigOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearPathConfigOutputResponse> {
        Self::clear_path_config(&self, req).await
    }
}
