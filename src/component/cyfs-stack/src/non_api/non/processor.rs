use super::super::acl::*;
use super::super::handler::*;
use super::fail_handler::NONOutputFailHandleProcessor;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::router_handler::RouterHandlersManager;
use crate::{acl::*, non::*};
use cyfs_base::*;
use cyfs_lib::*;

use std::convert::TryFrom;
use std::sync::Arc;

pub(crate) struct NONLevelInputProcessor {
    acl: AclManagerRef,
    noc: NONInputProcessorRef,
    forward: ForwardProcessorManager,
    fail_handler: ObjectFailHandler,
    router_handlers: RouterHandlersManager,
}

impl NONLevelInputProcessor {
    pub(crate) fn new_raw(
        acl: AclManagerRef,
        // 使用无权限的noc processor
        raw_noc_processor: NONInputProcessorRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
        router_handlers: RouterHandlersManager,
    ) -> NONInputProcessorRef {
        let ret = Self {
            acl,
            noc: raw_noc_processor,
            forward,
            fail_handler,
            router_handlers,
        };

        Arc::new(Box::new(ret))
    }

    pub(crate) fn new_zone(
        acl: AclManagerRef,
        raw_noc_processor: NONInputProcessorRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
        router_handlers: RouterHandlersManager,
    ) -> NONInputProcessorRef {
        // 不带input acl的处理器
        let raw_processor = Self::new_raw(
            acl.clone(),
            raw_noc_processor,
            forward,
            fail_handler,
            router_handlers,
        );

         // should process with rmeta
         let rmeta_processor = NONGlobalStateMetaAclInputProcessor::new(acl.clone(), raw_processor);

        // 带同zone input acl的处理器
        let acl_processor = NONZoneAclInputProcessor::new(rmeta_processor);

        acl_processor
    }

    async fn get_forward(&self, target: DeviceId) -> BuckyResult<NONInputProcessorRef> {
        // 获取到目标的processor
        let requestor = self.forward.get(&target).await?;

        // 这里不指定dec_id，使用forward request里面的dec_id
        let processor = NONRequestor::new(None, requestor).into_processor();

        // 增加一层错误监测处理
        let processor =
            NONOutputFailHandleProcessor::new(target.clone(), self.fail_handler.clone(), processor);

        // 转换为input processor
        let input_processor = NONInputTransformer::new(processor);

        // 增加forward前置处理器
        let pre_processor = NONHandlerPreProcessor::new(
            RouterHandlerChain::PreForward,
            input_processor,
            self.router_handlers.clone(),
        );

        // 增加forward后置处理器
        let post_processor = NONHandlerPostProcessor::new(
            RouterHandlerChain::PostForward,
            pre_processor,
            self.router_handlers.clone(),
        );

        Ok(post_processor)
    }

    // non层级下，不指定target那就是本地协议栈
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
                    let msg = format!("invalid non target type: {}, {:?}", object_id, v);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                }
            },
            None => None,
        };

        Ok(ret)
    }

    async fn get_processor(&self, target: Option<&ObjectId>) -> BuckyResult<NONInputProcessorRef> {
        if let Some(device_id) = self.get_target(target)? {
            debug!("non target resolved: {:?} -> {}", target, device_id);
            let processor = self.get_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.noc.clone())
        }
    }

    pub async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        debug!("will put object to non: {}", req);

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.put_object(req).await
    }

    pub async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        debug!("will get object from non: {}", req);

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_object(req).await
    }

    pub async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        debug!("will post object from non: {}", req);

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.post_object(req).await
    }


    pub async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        debug!("will select object from non: {}", req);

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.select_object(req).await
    }

    pub async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        debug!("will delete object from non: {}", req);

        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.delete_object(req).await
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONLevelInputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        NONLevelInputProcessor::put_object(&self, req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        NONLevelInputProcessor::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        NONLevelInputProcessor::post_object(&self, req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        NONLevelInputProcessor::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        NONLevelInputProcessor::delete_object(&self, req).await
    }
}
