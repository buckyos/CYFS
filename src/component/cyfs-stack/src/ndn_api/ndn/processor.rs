use super::super::acl::*;
use super::super::handler::*;
use super::super::ndc::NDCLevelInputProcessor;
use super::super::ndc::NDNObjectLoader;
use super::forward::NDNForwardDataOutputProcessor;
use super::forward_object_processor::NDNForwardObjectProcessor;
use super::NDNOutputFailHandleProcessor;
use crate::acl::*;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::ndn::*;
use crate::non::*;
use crate::router_handler::RouterHandlersManager;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_chunk_cache::ChunkManagerRef;
use cyfs_lib::*;
use cyfs_util::cache::NamedDataCache;

use std::convert::TryFrom;
use std::sync::Arc;

/*
ndn 依赖的non对象加载
1. 先通过无acl的本地noc加载
2. 再通过带zone acl的non向target加载
*/
pub(crate) struct NDNLevelInputProcessor {
    acl: AclManagerRef,

    bdt_stack: StackGuard,

    chunk_manager: ChunkManagerRef,

    // 对象加载器
    local_object_loader: NDNObjectLoader,
    target_object_loader: NDNObjectLoader,

    // local ndn
    ndc_processor: NDNInputProcessorRef,

    router_handlers: RouterHandlersManager,

    forward: ForwardProcessorManager,
    fail_handler: ObjectFailHandler,
}

impl NDNLevelInputProcessor {
    fn new_raw(
        acl: AclManagerRef,
        bdt_stack: StackGuard,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        raw_noc_processor: NONInputProcessorRef,
        inner_non_processor: NONInputProcessorRef,
        router_handlers: RouterHandlersManager,
        chunk_manager: ChunkManagerRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> NDNInputProcessorRef {
        let ndc_processor = NDCLevelInputProcessor::new_raw(
            chunk_manager.clone(),
            ndc,
            tracker,
            raw_noc_processor.clone(),
        );

        // 第一个是本地查询器，第二个是远程查询器
        let local_object_loader = NDNObjectLoader::new(raw_noc_processor);
        let target_object_loader = NDNObjectLoader::new(inner_non_processor);

        let ret = Self {
            acl,
            bdt_stack,
            chunk_manager,
            local_object_loader,
            target_object_loader,
            ndc_processor,
            router_handlers,
            forward,
            fail_handler,
        };

        Arc::new(Box::new(ret))
    }

    pub(crate) fn new_zone(
        acl: AclManagerRef,
        bdt_stack: StackGuard,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        raw_noc_processor: NONInputProcessorRef,
        inner_non_processor: NONInputProcessorRef,
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
            raw_noc_processor,
            inner_non_processor,
            router_handlers,
            chunk_manager,
            forward,
            fail_handler,
        );

        // 带同zone input acl的处理器
        let acl_processor = NDNAclInnerInputProcessor::new(acl, raw_processor.clone());

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

        // 限定同zone output权限
        let processor =
            NDNAclInnerOutputProcessor::new(self.acl.clone(), target.clone(), processor);

        // 增加前置的object加载器
        // 先尝试从本地加载, 再通过non从远程加载
        let processor = NDNForwardObjectProcessor::new(
            target,
            Some(self.local_object_loader.clone()),
            self.target_object_loader.clone(),
            processor,
        );

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

        // 限定同zone output权限
        let processor =
            NDNAclInnerOutputProcessor::new(self.acl.clone(), target.clone(), input_processor);

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
    fn get_target(&self, target: Option<&ObjectId>) -> BuckyResult<Option<DeviceId>> {
        let ret = match target {
            Some(object_id) => match object_id.obj_type_code() {
                ObjectTypeCode::Device => {
                    let device_id = DeviceId::try_from(object_id).map_err(|e| {
                        let msg = format!("invalid non target device_id: {}, {}", object_id, e);
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                    })?;

                    if device_id == *self.acl.get_current_device_id() {
                        None
                    } else {
                        Some(device_id)
                    }
                }
                v @ _ => {
                    let msg = format!("invalid ndn target type: {}, {:?}", object_id, v);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                }
            },
            None => None,
        };

        Ok(ret)
    }

    async fn get_data_processor(
        &self,
        target: Option<&ObjectId>,
    ) -> BuckyResult<NDNInputProcessorRef> {
        if let Some(device_id) = self.get_target(target)? {
            let processor = self.get_data_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.ndc_processor.clone())
        }
    }

    async fn get_processor(&self, target: Option<&ObjectId>) -> BuckyResult<NDNInputProcessorRef> {
        if let Some(device_id) = self.get_target(target)? {
            let processor = self.get_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.ndc_processor.clone())
        }
    }

    // put_data目前只支持chunk
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        debug!("will put data to ndn: {}", req);

        let processor = self.get_data_processor(req.common.target.as_ref()).await?;
        processor.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        debug!("will get data from ndn: {}", req);

        let processor = self.get_data_processor(req.common.target.as_ref()).await?;
        processor.get_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        debug!("will delete data from ndn: {}", req);

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        debug!("will query file from ndn: {}", req);

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.query_file(req).await
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNLevelInputProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        NDNLevelInputProcessor::put_data(&self, req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        NDNLevelInputProcessor::get_data(&self, req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        NDNLevelInputProcessor::delete_data(&self, req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        NDNLevelInputProcessor::query_file(&self, req).await
    }
}
