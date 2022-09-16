use super::super::meta::MetaInputProcessor;
use super::super::noc::*;
use super::super::non::*;
use super::super::router::*;
use crate::forward::ForwardProcessorManager;
use crate::meta::{MetaCache, ObjectFailHandler};
use crate::ndn_api::*;
use crate::resolver::OodResolver;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManagerRef;
use crate::{acl::*, non::*};
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_chunk_cache::ChunkManagerRef;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct NONService {
    raw_noc_processor: NONInputProcessorRef,
    noc: NONInputProcessorRef,
    non: NONInputProcessorRef,
    router: NONInputProcessorRef,
}

impl NONService {
    pub(crate) fn new(
        noc: NamedObjectCacheRef,
        bdt_stack: StackGuard,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        forward_manager: ForwardProcessorManager,
        acl: AclManagerRef,
        zone_manager: ZoneManagerRef,
        ood_resovler: OodResolver,

        router_handlers: RouterHandlersManager,
        meta_cache: Box<dyn MetaCache>,
        fail_handler: ObjectFailHandler,
        chunk_manager: ChunkManagerRef,
    ) -> (NONService, NDNService) {
        // 带file服务的无权限的noc processor
        let raw_noc_processor = NOCLevelInputProcessor::new_raw_with_file_service(
            noc.clone(),
            ndc.clone(),
            tracker.clone(),
            ood_resovler.clone(),
            router_handlers.clone(),
            chunk_manager.clone(),
        );

        // meta处理器，从mete和noc处理get_object请求
        let meta_processor = MetaInputProcessor::new_raw_with_file_service(
            None,
            meta_cache,
            ndc.clone(),
            tracker.clone(),
            ood_resovler.clone(),
            chunk_manager.clone(),
            noc.clone(),
        );

        // 带本地权限的noc processor
        let local_noc_processor =
            NOCLevelInputProcessor::new_local(raw_noc_processor.clone());

        // 同zone权限的non processor
        let non_processor = NONLevelInputProcessor::new_zone(
            acl.clone(),
            raw_noc_processor.clone(),
            forward_manager.clone(),
            fail_handler.clone(),
            router_handlers.clone(),
        );

        // 标准acl权限的router
        let router = NONRouter::new_acl(
            raw_noc_processor.clone(),
            forward_manager.clone(),
            acl.clone(),
            zone_manager.clone(),
            router_handlers.clone(),
            meta_processor,
            fail_handler.clone(),
        );

        let non_service = Self {
            raw_noc_processor: raw_noc_processor.clone(),
            noc: local_noc_processor,
            non: non_processor.clone(),
            router: router.clone(),
        };

        // 同时初始化ndn
        let ndn_service = NDNService::new(
            acl,
            bdt_stack,
            ndc,
            tracker.clone(),
            ood_resovler,
            zone_manager,
            router_handlers.clone(),
            raw_noc_processor,
            non_processor.clone(),
            router,
            chunk_manager.clone(),
            forward_manager,
            fail_handler,
        );

        (non_service, ndn_service)
    }

    pub(crate) fn raw_noc_processor(&self) -> &NONInputProcessorRef {
        &self.raw_noc_processor
    }

    pub(crate) fn clone_processor(&self) -> NONInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    fn get_api(&self, level: &NONAPILevel) -> &NONInputProcessorRef {
        match level {
            NONAPILevel::NOC => &self.noc,
            NONAPILevel::NON => &self.non,
            NONAPILevel::Router => &self.router,
        }
    }

    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        let processor = self.get_api(&req.common.level);
        processor.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let processor = self.get_api(&req.common.level);
        processor.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        let processor = self.get_api(&req.common.level);
        processor.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        let processor = self.get_api(&req.common.level);
        processor.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        let processor = self.get_api(&req.common.level);
        processor.delete_object(req).await
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONService {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        NONService::put_object(&self, req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        NONService::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        NONService::post_object(&self, req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        NONService::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        NONService::delete_object(&self, req).await
    }
}
