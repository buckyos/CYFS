use super::super::acl::*;
use super::super::handler::*;
use super::super::inner_path::NONInnerPathServiceProcessor;
use super::super::validate::NONGlobalStateValidatorProcessor;
use super::handler::NONRouterHandler;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManagerRef;
use crate::{non::*, AclManagerRef};
use cyfs_base::*;
use cyfs_lib::*;
use crate::NamedDataComponents;

use std::sync::Arc;

pub(crate) struct NOCLevelInputProcessor {
    noc: NamedObjectCacheRef,

    // action's handler with router handler system, now only valid for post_object
    handler: NONRouterHandler,
}

impl NOCLevelInputProcessor {
    fn new(
        zone_manager: ZoneManagerRef,
        router_handlers: &RouterHandlersManager,
        noc: NamedObjectCacheRef,
    ) -> NONInputProcessorRef {
        let handler = NONRouterHandler::new(&router_handlers, zone_manager);
        let ret = Self { noc, handler };
        Arc::new(Box::new(ret))
    }

    // noc processor with inner_path service
    pub(crate) fn new_with_inner_path_service(
        noc: NamedObjectCacheRef,
        relation: NamedObjectRelationCacheRef,
        named_data_components: &NamedDataComponents,
        router_handlers: RouterHandlersManager,
        zone_manager: ZoneManagerRef,
    ) -> NONInputProcessorRef {
        let raw_processor = Self::new(zone_manager, &router_handlers, noc.clone());

        // add inner_path supports
        let inner_path_processor =
            NONInnerPathServiceProcessor::new(raw_processor, named_data_components, noc, relation);

        // 增加pre-noc前置处理器
        let pre_processor = NONHandlerPreProcessor::new(
            RouterHandlerChain::PreNOC,
            inner_path_processor,
            router_handlers.clone(),
        );

        // 增加post-noc后置处理器
        let post_processor = NONHandlerPostProcessor::new(
            RouterHandlerChain::PostNOC,
            pre_processor,
            router_handlers.clone(),
        );

        post_processor
    }

    // processor with rmeta acl + valdiate
    pub(crate) fn new_rmeta_acl(
        acl: AclManagerRef,
        raw_processor: NONInputProcessorRef,
    ) -> NONInputProcessorRef {
        // should use validate for req_path
        let validate_noc_processor = NONGlobalStateValidatorProcessor::new(
            acl.global_state_validator().clone(),
            raw_processor.clone(),
        );

        // should process with rmeta
        let rmeta_processor = NONGlobalStateMetaAclInputProcessor::new(acl, validate_noc_processor);

        rmeta_processor
    }

    // processor with local device acl + rmeta acl + valdiate
    pub(crate) fn new_local_rmeta_acl(
        acl: AclManagerRef,
        raw_processor: NONInputProcessorRef,
    ) -> NONInputProcessorRef {
        // should process with rmeta
        let rmeta_processor = Self::new_rmeta_acl(acl, raw_processor);

        // only allowed on current device
        let acl_processor = NONLocalAclInputProcessor::new(rmeta_processor);

        acl_processor
    }

    pub async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        debug!(
            "will put object to local noc: id={}, access={:?}, {}",
            req.object.object_id, req.access, req.common.source,
        );

        let noc_req = NamedObjectCachePutObjectRequest {
            source: req.common.source,
            object: req.object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: req.access.as_ref().map(|v| v.value()),
        };

        let resp = match self.noc.put_object(&noc_req).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCachePutObjectResult::Accept => {
                        info!(
                            "put object to local noc success: id={}, access={:?}",
                            noc_req.object.object_id, req.access,
                        );
                    }
                    NamedObjectCachePutObjectResult::Updated => {
                        info!(
                            "object alreay in noc and updated: id={}, access={:?}",
                            noc_req.object.object_id, req.access
                        );
                    }
                    NamedObjectCachePutObjectResult::AlreadyExists => {
                        // 对象已经在noc里面了
                        info!(
                            "object alreay in noc and update time is newer: id={}, access={:?}",
                            noc_req.object.object_id, req.access
                        );
                    }
                    NamedObjectCachePutObjectResult::Merged => {
                        info!(
                            "object alreay in noc and signs merged: id={}, access={:?}",
                            noc_req.object.object_id, req.access,
                        );
                    }
                }

                Ok(resp)
            }
            Err(e) => {
                match e.code() {
                    BuckyErrorCode::Ignored => {
                        warn!(
                            "put object to local noc but been ignored: id={}, access={:?}, {}",
                            noc_req.object.object_id, req.access, e
                        );
                    }

                    BuckyErrorCode::Reject => {
                        warn!(
                            "put object to local noc but been rejected: id={}, access={:?}, {}",
                            noc_req.object.object_id, req.access, e
                        );
                    }

                    _ => {
                        error!(
                            "put object to local noc failed: id={}, access={:?}, {}",
                            noc_req.object.object_id, req.access, e
                        );
                    }
                }

                Err(e)
            }
        }?;

        // 返回对象的两个时间
        Ok(NONPutObjectInputResponse {
            result: resp.result.into(),
            object_expires_time: resp.expires_time,
            object_update_time: resp.update_time,
        })
    }

    async fn update_object_meta(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        assert!(req.object.is_empty());

        debug!(
            "will update object meta local noc: id={}, access={:?}, {}",
            req.object.object_id, req.access, req.common.source,
        );

        let noc_req = NamedObjectCacheUpdateObjectMetaRequest {
            source: req.common.source,
            object_id: req.object.object_id,
            storage_category: None,
            context: None,
            last_access_rpath: None,
            access_string: req.access.as_ref().map(|v| v.value()),
        };

        self.noc.update_object_meta(&noc_req).await?;

        info!(
            "update object meta success: id={}, access={:?}",
            noc_req.object_id, req.access,
        );

        Ok(NONPutObjectInputResponse {
            result: NONPutObjectResult::Accept,
            object_expires_time: None,
            object_update_time: None,
        })
    }

    pub async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            source: req.common.source,
            object_id: req.object_id.clone(),
            last_access_rpath: None,
        };

        match self.noc.get_object(&noc_req).await {
            Ok(Some(resp)) => {
                let mut resp = NONGetObjectInputResponse::new_with_object(resp.object);
                resp.init_times()?;

                Ok(resp)
            }
            Ok(None) => {
                let msg = format!("noc get object but not found: {}", req.object_id);
                debug!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
            Err(e) => Err(e),
        }
    }

    pub async fn select_object(
        &self,
        _req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        let msg = format!("select_object not yet supported!");
        error!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
    }

    pub async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        let noc_req = NamedObjectCacheDeleteObjectRequest {
            object_id: req.object_id.clone(),
            source: req.common.source,
            flags: req.common.flags,
        };

        match self.noc.delete_object(&noc_req).await {
            Ok(ret) => {
                let mut resp = NONDeleteObjectInputResponse { object: None };

                if req.common.flags & CYFS_REQUEST_FLAG_DELETE_WITH_QUERY != 0 {
                    if let Some(data) = ret.object {
                        assert!(data.object.is_some());
    
                        resp.object = Some(data);
                    }
                }
                
                Ok(resp)
            }
            Err(e) => Err(e),
        }
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NOCLevelInputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        if req.object.is_empty() {
            Self::update_object_meta(&self, req).await
        } else {
            Self::put_object(&self, req).await
        }
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        Self::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        self.handler.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        Self::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        Self::delete_object(&self, req).await
    }
}
