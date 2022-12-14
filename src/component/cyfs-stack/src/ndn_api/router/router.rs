use super::super::handler::*;
use super::super::ndc::*;
use super::super::ndn::*;
use super::super::forward::*;
use crate::NamedDataComponents;
use crate::acl::*;
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

use cyfs_chunk_cache::ChunkManagerRef;
use std::sync::Arc;

pub(crate) struct NDNRouter {
    acl: AclManagerRef,

    bdt_stack: StackGuard,

    chunk_manager: ChunkManagerRef,

    // local ndn
    ndc_processor: NDNInputProcessorRef,

    // object_loader
    object_loader: NDNObjectLoader,

    ood_resolver: OodResolver,
    zone_manager: ZoneManagerRef,

    router_handlers: RouterHandlersManager,

    // 用以实现转发请求
    forward: ForwardProcessorManager,
    fail_handler: ObjectFailHandler,
}

impl NDNRouter {
    fn new(
        acl: AclManagerRef,
        bdt_stack: StackGuard,
        named_data_components: &NamedDataComponents,
        non_router: NONInputProcessorRef,
        ood_resolver: OodResolver,
        zone_manager: ZoneManagerRef,
        router_handlers: RouterHandlersManager,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> NDNInputProcessorRef {
        // 使用router加载目标file
        let object_loader = NDNObjectLoader::new(non_router.clone());

        // local的ndn也使用router加载file
        let ndc_processor =
            NDCLevelInputProcessor::new(acl.clone(), named_data_components, non_router);

        let ret = Self {
            acl,
            bdt_stack,
            chunk_manager: named_data_components.chunk_manager.clone(),
            object_loader,
            ndc_processor,
            ood_resolver,
            zone_manager,
            router_handlers,
            forward,
            fail_handler,
        };

        Arc::new(Box::new(ret))
    }

    pub(crate) fn new_acl(
        acl: AclManagerRef,
        bdt_stack: StackGuard,
        named_data_components: &NamedDataComponents,
        non_router: NONInputProcessorRef,
        ood_resolver: OodResolver,
        zone_manager: ZoneManagerRef,
        router_handlers: RouterHandlersManager,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> NDNInputProcessorRef {
        // 不带input acl的处理器
        let processor = Self::new(
            acl.clone(),
            bdt_stack,
            named_data_components,
            non_router,
            ood_resolver,
            zone_manager,
            router_handlers,
            forward,
            fail_handler,
        );

        processor
    }

    async fn get_data_forward(&self, target: DeviceId) -> BuckyResult<NDNInputProcessorRef> {
        // ensure target device in local, used for bdt stack
        self.forward.get(&target).await?;

        // 获取到目标的processor
        let processor = NDNForwardDataOutputProcessor::new(
            self.bdt_stack.clone(),
            self.chunk_manager.clone(),
            target.clone(),
        );

        // 使用non router加载file
        let processor =
            NDNForwardObjectProcessor::new(target,self.object_loader.clone(), processor);

        // 增加forward前置处理器
        let pre_processor = NDNHandlerPreProcessor::new(
            RouterHandlerChain::PreForward,
            processor,
            self.router_handlers.clone(),
        );

        // 增加forward后置处理器
        let post_processor = NDNHandlerPostProcessor::new(
            RouterHandlerChain::PostForward,
            pre_processor,
            self.router_handlers.clone(),
        );

        Ok(post_processor)
    }

    // NDN resolve target logic is same as NON
    // is target = current_device, return NONE
    async fn resolve_target(&self, target: Option<&ObjectId>) -> BuckyResult<Option<DeviceId>> {
        let info = self.zone_manager.target_zone_manager().resolve_target(target).await?;
        let ret = if info.target_device == *self.acl.get_current_device_id() {
            None
        } else {
            Some(info.target_device)
        };

        info!("resolve ndn target: {:?} => {:?}", target, ret);
        Ok(ret)
    }

    // resolve final device from common.target param
    async fn get_data_processor(
        &self,
        target: Option<&ObjectId>,
    ) -> BuckyResult<NDNInputProcessorRef> {
        if let Some(device_id) = self.resolve_target(target).await? {
            let processor = self.get_data_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.ndc_processor.clone())
        }
    }

    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        debug!(
            "will put data to ndn: id={}, {}, target={:?}",
            req.object_id, req.common.source, req.common.target,
        );

        let processor = self.get_data_processor(req.common.target.as_ref()).await?;
        processor.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        debug!(
            "will get data from ndn: id={}, {}, target={:?}",
            req.object_id, req.common.source, req.common.target
        );

        let processor = self.get_data_processor(req.common.target.as_ref()).await?;
        processor.get_data(req).await
    }

    // for NONE data processor， just forward the request as non does
    async fn get_forward(&self, target: DeviceId) -> BuckyResult<NDNInputProcessorRef> {
        // 获取到目标的processor
        let requestor = self.forward.get(&target).await?;

        // 这里不指定dec_id，使用forward request里面的dec_id
        let processor = NDNRequestor::new(None, requestor).into_processor();

        // 增加一层错误监测处理
        let processor =
            NDNOutputFailHandleProcessor::new(target.clone(), self.fail_handler.clone(), processor);

        // 转换为input processor
        let input_processor = NDNInputTransformer::new(processor);

        // 增加forward前置处理器
        let pre_processor = NDNHandlerPreProcessor::new(
            RouterHandlerChain::PreForward,
            input_processor,
            self.router_handlers.clone(),
        );

        // 增加forward后置处理器
        let post_processor = NDNHandlerPostProcessor::new(
            RouterHandlerChain::PostForward,
            pre_processor,
            self.router_handlers.clone(),
        );

        Ok(post_processor)
    }

    async fn get_processor(&self, target: Option<&ObjectId>) -> BuckyResult<NDNInputProcessorRef> {
        if let Some(device_id) = self.resolve_target(target).await? {
            let processor = self.get_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.ndc_processor.clone())
        }
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        debug!(
            "will delete data from ndn: id={}, {}, target={:?}",
            req.object_id, req.common.source, req.common.target,
        );

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        debug!(
            "will query file from ndn: param={}, {}, target={:?}",
            req.param, req.common.source, req.common.target,
        );

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.query_file(req).await
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNRouter {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        Self::put_data(&self, req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        Self::get_data(&self, req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        Self::delete_data(&self, req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        Self::query_file(&self, req).await
    }
}
