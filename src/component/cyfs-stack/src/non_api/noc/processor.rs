use super::super::acl::*;
use super::super::file::NONFileServiceProcessor;
use super::super::handler::*;
use crate::ndn_api::NDCLevelInputProcessor;
use crate::resolver::OodResolver;
use crate::router_handler::RouterHandlersManager;
use crate::{acl::*, non::*};
use cyfs_base::*;
use cyfs_lib::*;

use cyfs_chunk_cache::ChunkManager;
use std::sync::Arc;

pub(crate) struct NOCLevelInputProcessor {
    noc: NamedObjectCacheRef,
}

impl NOCLevelInputProcessor {
    fn new_raw(noc: NamedObjectCacheRef) -> NONInputProcessorRef {
        let ret = Self { noc };
        Arc::new(Box::new(ret))
    }

    // 带file服务的noc processor
    pub(crate) fn new_raw_with_file_service(
        noc: NamedObjectCacheRef,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        ood_resolver: OodResolver,
        router_handlers: RouterHandlersManager,
        chunk_manager: Arc<ChunkManager>,
    ) -> NONInputProcessorRef {
        let raw_processor = Self::new_raw(noc.clone());

        let ndc =
            NDCLevelInputProcessor::new_raw(chunk_manager, ndc, tracker, raw_processor.clone());

        let file_processor =
            NONFileServiceProcessor::new(NONAPILevel::NOC, raw_processor, ndc, ood_resolver, noc);

        // 增加pre-noc前置处理器
        let pre_processor = NONHandlerPreProcessor::new(
            RouterHandlerChain::PreNOC,
            file_processor,
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

    // 创建一个带本地权限的processor
    pub(crate) fn new_local(
        acl: AclManagerRef,
        raw_processor: NONInputProcessorRef,
    ) -> NONInputProcessorRef {
        // 带local input acl的处理器
        let acl_processor = NONAclLocalInputProcessor::new(acl, raw_processor.clone());

        // 使用acl switcher连接
        let processor = NONInputAclSwitcher::new(acl_processor, raw_processor);

        processor
    }

    pub async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        debug!(
            "will put object to local noc: id={}, source={}, dec={:?}",
            req.object.object_id, req.common.source, req.common.dec_id,
        );

        let source = RequestSourceInfo::new_local_dec(req.common.dec_id);

        let noc_req = NamedObjectCachePutObjectRequest {
            source,
            object: req.object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: None,
        };

        let resp = match self.noc.put_object(&noc_req).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCachePutObjectResult::Accept => {
                        info!(
                            "put object to local noc success: id={}",
                            noc_req.object.object_id
                        );
                    }
                    NamedObjectCachePutObjectResult::Updated => {
                        info!("object alreay in noc and updated: {}", noc_req.object.object_id);
                    }
                    NamedObjectCachePutObjectResult::AlreadyExists => {
                        // 对象已经在noc里面了
                        info!("object alreay in noc: {}", noc_req.object.object_id);
                    }
                    NamedObjectCachePutObjectResult::Merged => {
                        info!(
                            "object alreay in noc and signs merged: {}",
                            noc_req.object.object_id
                        );
                    }
                }

                Ok(resp)
            }
            Err(e) => {
                match e.code() {
                    BuckyErrorCode::Ignored => {
                        warn!(
                            "put object to local noc but been ignored: id={}, {}",
                            noc_req.object.object_id, e
                        );
                    }

                    BuckyErrorCode::Reject => {
                        warn!(
                            "put object to local noc but been rejected: id={}, {}",
                            noc_req.object.object_id, e
                        );
                    }

                    _ => {
                        error!(
                            "put object to local noc failed: id={}, {}",
                            noc_req.object.object_id, e
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

    pub async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let source = RequestSourceInfo::new_local_dec(req.common.dec_id);

        let noc_req = NamedObjectCacheGetObjectRequest {
            source,
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
            source: RequestSourceInfo::new_local_dec(req.common.dec_id),
            flags: req.common.flags,
        };

        match self.noc.delete_object(&noc_req).await {
            Ok(ret) => {
                let mut resp = NONDeleteObjectInputResponse { object: None };

                if let Some(data) = ret.object {
                    assert!(data.object.is_some());

                    resp.object = Some(data);
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
        NOCLevelInputProcessor::put_object(&self, req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        NOCLevelInputProcessor::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        let msg = format!(
            "post_object not support on noc level! id={}",
            req.object.object_id
        );
        error!("{}", msg);

        Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        NOCLevelInputProcessor::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        NOCLevelInputProcessor::delete_object(&self, req).await
    }
}
