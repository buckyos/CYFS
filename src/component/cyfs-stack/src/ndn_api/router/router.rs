use super::super::acl::*;
use super::super::handler::*;
use super::super::ndc::*;
use super::super::ndn::*;
use crate::acl::*;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::ndn::*;
use crate::non::*;
use crate::resolver::OodResolver;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManager;
use cyfs_util::cache::NamedDataCache;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_bdt::StackGuard;

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
    zone_manager: ZoneManager,

    router_handlers: RouterHandlersManager,

    // 用以实现转发请求
    forward: ForwardProcessorManager,
    fail_handler: ObjectFailHandler,
}

impl NDNRouter {
    fn new_raw(
        acl: AclManagerRef,
        bdt_stack: StackGuard,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        non_router: NONInputProcessorRef,
        ood_resolver: OodResolver,
        zone_manager: ZoneManager,
        router_handlers: RouterHandlersManager,
        chunk_manager: ChunkManagerRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> NDNInputProcessorRef {
        // 使用router加载目标file
        let object_loader = NDNObjectLoader::new(non_router.clone());

        // local的ndn也使用router加载file
        let ndc_processor =
            NDCLevelInputProcessor::new_raw(chunk_manager.clone(), ndc, tracker, non_router);
        let ret = Self {
            acl,
            bdt_stack,
            chunk_manager,
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
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        non_router: NONInputProcessorRef,
        ood_resolver: OodResolver,
        zone_manager: ZoneManager,
        router_handlers: RouterHandlersManager,
        chunk_manager: ChunkManagerRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> NDNInputProcessorRef {
        // 不带input acl的处理器
        let raw_processor = Self::new_raw(
            acl.clone(),
            bdt_stack,
            ndc,
            tracker,
            non_router,
            ood_resolver,
            zone_manager,
            router_handlers,
            chunk_manager,
            forward,
            fail_handler,
        );

        // 带input acl的处理器
        let acl_processor = NDNAclInputProcessor::new(acl, raw_processor.clone());

        // 使用acl switcher连接
        let processor = NDNInputAclSwitcher::new(acl_processor, raw_processor);

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

        // 限定标准acl权限
        let processor = NDNAclOutputProcessor::new(self.acl.clone(), target.clone(), processor);

        // 使用non router加载file
        let processor =
            NDNForwardObjectProcessor::new(target, None, self.object_loader.clone(), processor);

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

    // ndn层级下，不指定target那就是本地协议栈
    async fn resolve_target(&self, target: Option<&ObjectId>) -> BuckyResult<Option<DeviceId>> {
        let (_, final_target) = self.zone_manager.resolve_target(target, None).await?;
        let ret = if final_target == *self.acl.get_current_device_id() {
            None
        } else {
            Some(final_target)
        };

        info!("resolve ndn target: {:?} => {:?}", target, ret);
        Ok(ret)
    }

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

    // 从目标对象，解析出所在的源设备
    async fn resolve_target_from_object(
        &self,
        req: &NDNGetDataInputRequest,
    ) -> BuckyResult<Vec<DeviceId>> {
        // 只需要查询object_id，不附带inner_path
        let mut obj_req = req.clone();
        obj_req.inner_path = None;
        let object_info = self.object_loader.get_root_object(&obj_req, None).await?;

        self.search_source_from_object(object_info).await
    }

    async fn search_source_from_object(
        &self,
        object_info: NONObjectInfo,
    ) -> BuckyResult<Vec<DeviceId>> {
        let mut sources = vec![];
        match self
            .ood_resolver
            .get_ood_by_object(
                object_info.object_id.clone(),
                None,
                object_info.object.unwrap(),
            )
            .await
        {
            Ok(list) => {
                if list.is_empty() {
                    info!(
                        "get sources from file|dir owner but not found! file={}",
                        object_info.object_id,
                    );
                } else {
                    info!(
                        "get sources from file|dir owner! file={}, sources={:?}",
                        object_info.object_id, list
                    );

                    list.into_iter().for_each(|device_id| {
                        // 这里需要列表去重
                        if !sources.iter().any(|v| *v == device_id) {
                            sources.push(device_id);
                        }
                    });
                }

                Ok(sources)
            }
            Err(e) => {
                error!(
                    "get sources from file|dir owner failed! file={}",
                    object_info.object_id,
                );
                Err(e)
            }
        }
    }

    // put_data 直接put到target目标设备(如果=None则put到本地)
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        debug!(
            "will put data to ndn: id={}, source={}, target={:?} dec={:?}",
            req.object_id, req.common.source, req.common.target, req.common.dec_id,
        );

        let processor = self.get_data_processor(req.common.target.as_ref()).await?;
        processor.put_data(req).await
    }

    async fn resolve_get_data_target(&self, req: &NDNGetDataInputRequest) -> Option<ObjectId> {
        // 如果明确指定了target，那么尝试从target拉取
        if let Some(target) = &req.common.target {
            Some(target.to_owned())
        } else {
            // 从目标对象解析target
            if let Ok(list) = self.resolve_target_from_object(req).await {
                info!(
                    "ndn resolve get data target: object={}, targets={:?}",
                    req.object_id, list
                );
                if list.len() > 0 {
                    Some(list[0].object_id().to_owned())
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        debug!(
            "will get data from ndn: id={}, source={}, target={:?}, dec={:?}",
            req.object_id, req.common.source, req.common.target, req.common.dec_id,
        );

        // 从req.target和目标对象，解析出所在的ood
        let target = self.resolve_get_data_target(&req).await;

        let processor = self.get_data_processor(target.as_ref()).await?;
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

        // 限定标准acl权限
        let processor =
            NDNAclOutputProcessor::new(self.acl.clone(), target.clone(), input_processor);

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
            "will delete data from ndn: id={}, source={}, target={:?} dec={:?}",
            req.object_id, req.common.source, req.common.target, req.common.dec_id,
        );

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        debug!(
            "will query file from ndn: param={}, source={}, target={:?} dec={:?}",
            req.param, req.common.source, req.common.target, req.common.dec_id,
        );

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.query_file(req).await
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNRouter {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        NDNRouter::put_data(&self, req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        NDNRouter::get_data(&self, req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        NDNRouter::delete_data(&self, req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        NDNRouter::query_file(&self, req).await
    }
}
