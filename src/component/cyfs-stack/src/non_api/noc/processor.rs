use super::super::acl::*;
use super::super::file::NONFileServiceProcessor;
use super::super::handler::*;
use crate::ndn_api::NDCLevelInputProcessor;
use crate::router_handler::RouterHandlersManager;
use crate::resolver::OodResolver;
use crate::{acl::*, non::*};
use cyfs_base::*;
use cyfs_lib::*;


use std::convert::TryFrom;
use std::sync::Arc;
use cyfs_chunk_cache::ChunkManager;

pub(crate) struct NOCLevelInputProcessor {
    noc: Box<dyn NamedObjectCache>,
}

impl NOCLevelInputProcessor {
    fn new_raw(noc: Box<dyn NamedObjectCache>) -> NONInputProcessorRef {
        let ret = Self { noc };
        Arc::new(Box::new(ret))
    }

    // 带file服务的noc processor
    pub(crate) fn new_raw_with_file_service(
        noc: Box<dyn NamedObjectCache>,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        ood_resolver: OodResolver,
        router_handlers: RouterHandlersManager,
        chunk_manager: Arc<ChunkManager>,
    ) -> NONInputProcessorRef {
        let raw_processor = Self::new_raw(noc.clone_noc());

        let ndc = NDCLevelInputProcessor::new_raw(chunk_manager, ndc, tracker, raw_processor.clone());

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

        let noc_req = NamedObjectCacheInsertObjectRequest {
            protocol: req.common.protocol,
            source: req.common.source.clone(),
            dec_id: req.common.dec_id.clone(),

            object: req.object.clone_object(),
            object_id: req.object.object_id,
            object_raw: req.object.object_raw,
            flags: req.common.flags,
        };

        let resp = match self.noc.insert_object(&noc_req).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCacheInsertResult::Accept => {
                        info!(
                            "put object to local noc success: id={}",
                            req.object.object_id
                        );
                    }
                    NamedObjectCacheInsertResult::Updated => {
                        info!("object alreay in noc and updated: {}", req.object.object_id);
                    }
                    NamedObjectCacheInsertResult::AlreadyExists => {
                        // 对象已经在noc里面了
                        info!("object alreay in noc: {}", req.object.object_id);
                    }
                    NamedObjectCacheInsertResult::Merged => {
                        info!(
                            "object alreay in noc and signs merged: {}",
                            req.object.object_id
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
                            req.object.object_id, e
                        );
                    }

                    BuckyErrorCode::Reject => {
                        warn!(
                            "put object to local noc but been rejected: id={}, {}",
                            req.object.object_id, e
                        );
                    }

                    _ => {
                        error!(
                            "put object to local noc failed: id={}, {}",
                            req.object.object_id, e
                        );
                    }
                }

                Err(e)
            }
        }?;

        // 返回对象的两个时间
        Ok(NONPutObjectInputResponse {
            result: resp.result.into(),
            object_expires_time: resp.object_expires_time,
            object_update_time: resp.object_update_time,
        })
    }

    pub async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            protocol: req.common.protocol,
            object_id: req.object_id.clone(),
            source: req.common.source,
        };

        match self.noc.get_object(&noc_req).await {
            Ok(Some(resp)) => {
                assert!(resp.object.is_some());
                assert!(resp.object_raw.is_some());

                let mut resp = NONGetObjectInputResponse::new(
                    req.object_id,
                    resp.object_raw.unwrap(),
                    resp.object,
                );
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
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        let filter = NamedObjectCacheSelectObjectFilter::from(req.filter.clone());
        let opt = match &req.opt {
            Some(opt) => Some(NamedObjectCacheSelectObjectOption::try_from(opt)?),
            None => None,
        };

        debug!(
            "will process select_object request: filter={:?}, opt={:?}",
            filter, opt
        );

        // 从noc查询
        let noc_req = NamedObjectCacheSelectObjectRequest {
            protocol: req.common.protocol,
            source: req.common.source,
            filter,
            opt,
        };

        let ret = self.noc.select_object(&noc_req).await?;

        // 对所有结果转换为目标类型
        let mut objects: Vec<SelectResponseObjectInfo> = Vec::new();
        for item in ret.into_iter() {
            let object = NONObjectInfo::new(item.object_id, item.object_raw.unwrap(), item.object);
            let resp_info = SelectResponseObjectInfo {
                size: object.object_raw.len() as u32,
                insert_time: item.insert_time,
                object: Some(object),
            };

            objects.push(resp_info);
        }

        Ok(NONSelectObjectInputResponse { objects })
    }

    pub async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        let noc_req = NamedObjectCacheDeleteObjectRequest {
            protocol: req.common.protocol,
            object_id: req.object_id.clone(),
            source: req.common.source,
            flags: req.common.flags,
        };

        match self.noc.delete_object(&noc_req).await {
            Ok(ret) => {
                let mut resp = NONDeleteObjectInputResponse { object: None };

                if let Some(data) = ret.object {
                    assert!(data.object.is_some());
                    assert!(data.object_raw.is_some());

                    let object =
                        NONObjectInfo::new(req.object_id, data.object_raw.unwrap(), data.object);
                    resp.object = Some(object);
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
