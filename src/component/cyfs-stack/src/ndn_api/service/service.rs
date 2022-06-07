use super::super::cache::NDNDataCacheManager;
use super::super::ndc::*;
use super::super::ndn::*;
use super::super::router::*;
use crate::acl::AclManagerRef;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::ndn::*;
use crate::non::*;
use crate::resolver::OodResolver;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManager;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_bdt::StackGuard;

use cyfs_chunk_cache::ChunkManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct NDNService {
    ndc: NDNInputProcessorRef,
    ndn: NDNInputProcessorRef,
    router: NDNInputProcessorRef,
    chunk_manager: Arc<ChunkManager>,
}

impl NDNService {
    pub(crate) fn new(
        acl: AclManagerRef,
        bdt_stack: StackGuard,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        data_cache: NDNDataCacheManager,

        ood_resolver: OodResolver,
        zone_manager: ZoneManager,
        router_handlers: RouterHandlersManager,

        // 不带权限的本地noc处理器
        raw_noc_processor: NONInputProcessorRef,

        // 带inner权限的non处理器
        inner_non_processor: NONInputProcessorRef,

        // 带acl的non router
        non_router: NONInputProcessorRef,
        chunk_manager: Arc<ChunkManager>,

        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> Self {
        let ndc_processor = NDCLevelInputProcessor::new_local(
            acl.clone(),
            chunk_manager.clone(),
            ndc.clone(),
            tracker.clone(),
            raw_noc_processor.clone(),
        );

        let ndn_processor = NDNLevelInputProcessor::new_zone(
            acl.clone(),
            bdt_stack.clone(),
            ndc.clone(),
            tracker.clone(),
            data_cache.clone(),
            raw_noc_processor.clone(),
            inner_non_processor,
            router_handlers.clone(),
            chunk_manager.clone(),
            forward.clone(),
            fail_handler.clone(),
        );

        let router = NDNRouter::new_acl(
            acl,
            bdt_stack,
            ndc,
            tracker,
            data_cache,
            non_router,
            ood_resolver,
            zone_manager,
            router_handlers,
            chunk_manager.clone(),
            forward,
            fail_handler,
        );

        Self {
            ndc: ndc_processor,
            ndn: ndn_processor,
            router,
            chunk_manager,
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
