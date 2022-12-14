use super::super::ndc::*;
use super::super::ndn::*;
use super::super::router::*;
use crate::NamedDataComponents;
use crate::acl::AclManagerRef;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::ndn::*;
use crate::non::*;
use crate::resolver::OodResolver;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct NDNService {
    ndc: NDNInputProcessorRef,
    ndn: NDNInputProcessorRef,
    router: NDNInputProcessorRef,
}

impl NDNService {
    pub(crate) fn new(
        acl: AclManagerRef,
        bdt_stack: StackGuard,
        named_data_components: &NamedDataComponents,

        ood_resolver: OodResolver,
        zone_manager: ZoneManagerRef,
        router_handlers: RouterHandlersManager,

        // 带acl的non router
        non_router: NONInputProcessorRef,

        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> Self {
        let ndc_processor = NDCLevelInputProcessor::new_local(
            acl.clone(),
            named_data_components,
            non_router.clone(),
        );

        let ndn_processor = NDNLevelInputProcessor::new_zone(
            acl.clone(),
            bdt_stack.clone(),
            named_data_components,
            non_router.clone(),
            router_handlers.clone(),
            forward.clone(),
            fail_handler.clone(),
        );

        let router = NDNRouter::new_acl(
            acl,
            bdt_stack,
            named_data_components,
            non_router,
            ood_resolver,
            zone_manager,
            router_handlers,
            forward,
            fail_handler,
        );

        Self {
            ndc: ndc_processor,
            ndn: ndn_processor,
            router,
        }
    }

    pub(crate) fn clone_processor(&self) -> NDNInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    pub(crate) fn get_api(&self, level: &NDNAPILevel) -> &NDNInputProcessorRef {
        match level {
            NDNAPILevel::NDC => &self.ndc,
            NDNAPILevel::NDN => &self.ndn,
            NDNAPILevel::Router => &self.router,
        }
    }

    // put_data目前只支持put到本地
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        let processor = self.get_api(&req.common.level);
        processor.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        if req.data_type == NDNDataType::SharedMem {
            if req.common.level != NDNAPILevel::NDC {
                let msg = format!(
                    "get_shared_data only support from NDC. id = {}",
                    req.object_id.to_string()
                );
                log::error!("{}", msg.as_str());
                return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
            }

            if req.object_id.obj_type_code() != ObjectTypeCode::Chunk {
                let msg = format!(
                    "get_shared_data only support from Chunk. id = {}",
                    req.object_id.to_string()
                );
                log::error!("{}", msg.as_str());
                return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
            }
        }
        let processor = self.get_api(&req.common.level);
        processor.get_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        let processor = self.get_api(&req.common.level);
        processor.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        let processor = self.get_api(&req.common.level);
        processor.query_file(req).await
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNService {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        NDNService::put_data(&self, req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        NDNService::get_data(&self, req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        NDNService::delete_data(&self, req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        NDNService::query_file(&self, req).await
    }
}
