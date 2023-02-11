use super::super::meta::MetaInputProcessor;
use super::super::noc::*;
use super::super::non::*;
use super::super::router::*;
use crate::NamedDataComponents;
use crate::forward::ForwardProcessorManager;
use crate::meta::{MetaCacheRef, ObjectFailHandler};
use crate::ndn_api::*;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManagerRef;
use crate::{acl::*, non::*};
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct NONService {
    raw_noc_processor: NONInputProcessorRef,
    noc: NONInputProcessorRef,
    rmeta_noc_processor: NONInputProcessorRef,
    non: NONInputProcessorRef,
    router: NONInputProcessorRef,
}

impl NONService {
    pub(crate) fn new(
        noc: NamedObjectCacheRef,
        bdt_stack: StackGuard,
        named_data_components: &NamedDataComponents,
        forward_manager: ForwardProcessorManager,
        acl: AclManagerRef,
        zone_manager: ZoneManagerRef,
        router_handlers: RouterHandlersManager,
        meta_cache: MetaCacheRef,
        fail_handler: ObjectFailHandler,
    ) -> (NONService, NDNService) {
        // raw service with inner_path service support
        let raw_noc_processor = NOCLevelInputProcessor::new_with_inner_path_service(
            noc.clone(),
            named_data_components,
            router_handlers.clone(),
            zone_manager.clone(),
        );

        // meta处理器，从mete和noc处理get_object请求
        let meta_processor = MetaInputProcessor::new_with_inner_path_service(
            None,
            meta_cache,
            named_data_components,
            noc.clone(),
        );

        // noc processor with local device acl + rmeta acl + validate
        let local_noc_processor =
            NOCLevelInputProcessor::new_local_rmeta_acl(acl.clone(), raw_noc_processor.clone());

        // noc processor only with rmeta acl + validate
        let rmeta_noc_processor =
            NOCLevelInputProcessor::new_rmeta_acl(acl.clone(), raw_noc_processor.clone());

        // non processor with zone acl + rmeta acl + validate
        let non_processor = NONLevelInputProcessor::new_zone(
            acl.clone(),
            raw_noc_processor.clone(),
            forward_manager.clone(),
            fail_handler.clone(),
            router_handlers.clone(),
        );

        // 标准acl权限的router + rmeta acl + validate
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
            rmeta_noc_processor,
            non: non_processor.clone(),
            router: router.clone(),
        };

        // 同时初始化ndn
        let ndn_service = NDNService::new(
            acl,
            bdt_stack,
            named_data_components,
            zone_manager,
            router_handlers.clone(),
            router,
            forward_manager,
            fail_handler,
        );

        (non_service, ndn_service)
    }

    pub(crate) fn raw_noc_processor(&self) -> &NONInputProcessorRef {
        &self.raw_noc_processor
    }

    pub(crate) fn rmeta_noc_processor(&self) -> &NONInputProcessorRef {
        &self.rmeta_noc_processor
    }

    pub(crate) fn router_processor(&self) -> &NONInputProcessorRef {
        &self.router
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
