use super::super::acl::*;
use super::super::forward::*;
use super::super::handler::*;
use super::super::ndc::NDCLevelInputProcessor;
use super::super::ndc::NDNObjectLoader;
use crate::acl::*;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::ndn::*;
use crate::non::*;
use crate::router_handler::RouterHandlersManager;
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_bdt_ext::{ContextManager, NDNTaskCancelStrategy, TransContextHolder};
use cyfs_chunk_cache::ChunkManagerRef;
use cyfs_lib::*;

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
    target_object_loader: NDNObjectLoader,

    // local ndn
    ndc_processor: NDNInputProcessorRef,

    router_handlers: RouterHandlersManager,

    forward: ForwardProcessorManager,
    fail_handler: ObjectFailHandler,

    context_manager: ContextManager,
}

impl NDNLevelInputProcessor {
    fn new(
        acl: AclManagerRef,
        bdt_stack: StackGuard,
        named_data_components: &NamedDataComponents,
        non_processor: NONInputProcessorRef,
        router_handlers: RouterHandlersManager,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> NDNInputProcessorRef {
        let ndc_processor =
            NDCLevelInputProcessor::new(acl.clone(), named_data_components, non_processor.clone());

        // target object loader
        let target_object_loader = NDNObjectLoader::new(non_processor);

        let ret = Self {
            acl,
            bdt_stack,
            chunk_manager: named_data_components.chunk_manager.clone(),
            target_object_loader,
            ndc_processor,
            router_handlers,
            forward,
            fail_handler,
            context_manager: named_data_components.context_manager.clone(),
        };

        Arc::new(Box::new(ret))
    }

    pub(crate) fn new_zone(
        acl: AclManagerRef,
        bdt_stack: StackGuard,
        named_data_components: &NamedDataComponents,
        non_processor: NONInputProcessorRef,
        router_handlers: RouterHandlersManager,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> NDNInputProcessorRef {
        // 不带input acl的处理器
        let processor = Self::new(
            acl,
            bdt_stack,
            named_data_components,
            non_processor,
            router_handlers,
            forward,
            fail_handler,
        );

        // 带同zone input acl的处理器
        let acl_processor = NDNAclZoneInputProcessor::new(processor);

        acl_processor
    }

    async fn get_data_forward(
        &self,
        context: TransContextHolder,
    ) -> BuckyResult<NDNInputProcessorRef> {
        // ensure target device in local, used for bdt stack
        // self.forward.get(&target).await?;

        let non_target = context.non_target().await.ok_or_else(|| {
            let msg = format!(
                "ndn get_file but non target not exists! {}",
                context.debug_string()
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        // 获取到目标的processor
        let processor = NDNForwardDataOutputProcessor::new(
            self.bdt_stack.clone(),
            self.chunk_manager.clone(),
            context,
        );

        // 增加前置的object加载器
        // 通过合适的non processor加载目标object
        let processor = NDNForwardObjectProcessor::new(
            non_target,
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
        let processor = NDNRequestor::new(None, requestor, None).into_processor();


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
        req: &NDNGetDataInputRequest,
    ) -> BuckyResult<NDNInputProcessorRef> {
        match &req.context {
            Some(context) => {
                let referer = BdtDataRefererInfo::from(req).encode_string();
                let context = self
                    .context_manager
                    .create_download_context_from_trans_context(
                        &req.common.source.dec,
                        referer,
                        context.as_str(),
                        NDNTaskCancelStrategy::AutoCancel,
                    )
                    .await?;
                let processor = self.get_data_forward(context).await?;
                Ok(processor)
            }
            None => {
                if let Some(device_id) = self.get_target(req.common.target.as_ref())? {
                    let referer = BdtDataRefererInfo::from(req).encode_string();
                    let context = self
                        .context_manager
                        .create_download_context_from_target(referer, device_id)
                        .await?;
                    let processor = self.get_data_forward(context).await?;
                    Ok(processor)
                } else {
                    Ok(self.ndc_processor.clone())
                }
            }
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

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        debug!("will get data from ndn: {}", req);

        let processor = self.get_data_processor(&req).await?;
        processor.get_data(req).await
    }

    // put_data目前只支持chunk
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        debug!("will put data to ndn: {}", req);

        if let Some(device_id) = self.get_target(req.common.target.as_ref())? {
            let msg = format!(
                "ndn put_data to target not support! chunk={}, target={}",
                req.object_id, device_id,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
        }

        self.ndc_processor.put_data(req).await
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
