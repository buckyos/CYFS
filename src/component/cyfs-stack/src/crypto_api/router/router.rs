use super::super::acl::*;
use super::super::handler::*;
use super::super::local::ObjectCrypto;
use super::fail_handler::CryptoOutputFailHandleProcessor;
use crate::acl::AclManagerRef;
use crate::crypto::*;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct CryptoRouter {
    acl: AclManagerRef,

    processor: CryptoInputProcessorRef,

    zone_manager: ZoneManagerRef,

    forward: ForwardProcessorManager,

    fail_handler: ObjectFailHandler,

    router_handlers: RouterHandlersManager,
}

impl CryptoRouter {
    fn new_raw(
        acl: AclManagerRef,
        processor: CryptoInputProcessorRef,
        zone_manager: ZoneManagerRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
        router_handlers: RouterHandlersManager,
    ) -> CryptoInputProcessorRef {
        let ret = Self {
            acl,
            processor,
            zone_manager,
            forward,
            fail_handler,
            router_handlers,
        };

        Arc::new(Box::new(ret))
    }

    fn new_local_with_handler(
        object_crypto: ObjectCrypto,
        router_handlers: &RouterHandlersManager,
    ) -> CryptoInputProcessorRef {
        let processor = object_crypto.clone_processor();

        // 增加pre-crypto前置处理器
        let pre_processor = CryptoHandlerPreProcessor::new(
            RouterHandlerChain::PreCrypto,
            processor,
            router_handlers.clone(),
        );

        // 增加post-crypto后置处理器
        let post_processor = CryptoHandlerPostProcessor::new(
            RouterHandlerChain::PostCrypto,
            pre_processor,
            router_handlers.clone(),
        );

        post_processor
    }

    pub(crate) fn new_acl(
        acl: AclManagerRef,
        object_crypto: ObjectCrypto,
        zone_manager: ZoneManagerRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
        router_handlers: RouterHandlersManager,
    ) -> CryptoInputProcessorRef {
        // 本地的crypto需要增加handler
        let processor = Self::new_local_with_handler(object_crypto, &router_handlers);

        let raw_router = Self::new_raw(
            acl.clone(),
            processor,
            zone_manager,
            forward,
            fail_handler,
            router_handlers,
        );

        let acl_router = CryptoAclInputProcessor::new(acl, raw_router.clone());

        acl_router
    }

    async fn get_forward(&self, target: DeviceId) -> BuckyResult<CryptoInputProcessorRef> {
        // 获取到目标的processor
        let requestor = self.forward.get(&target).await?;

        // 这里不指定dec_id，使用forward request里面的dec_id
        let processor = CryptoRequestor::new(None, requestor).into_processor();

        // 增加一层错误处理
        let processor = CryptoOutputFailHandleProcessor::new(
            target.clone(),
            self.fail_handler.clone(),
            processor,
        );

        // 转换为input processor
        let input_processor = CryptoInputTransformer::new(processor);

        // 增加forward前置处理器
        let pre_processor = CryptoHandlerPreProcessor::new(
            RouterHandlerChain::PreForward,
            input_processor,
            self.router_handlers.clone(),
        );

        // 增加forward后置处理器
        let post_processor = CryptoHandlerPostProcessor::new(
            RouterHandlerChain::PostForward,
            pre_processor,
            self.router_handlers.clone(),
        );

        Ok(post_processor)
    }

    // 不同于non/ndn的router，如果target为空，那么表示本地device
    async fn get_target(&self, target: Option<&ObjectId>) -> BuckyResult<Option<DeviceId>> {
        let ret = match target {
            Some(object_id) => {
                let info = self
                    .zone_manager
                    .target_zone_manager()
                    .resolve_target(Some(object_id))
                    .await?;
                if info.target_device == *self.zone_manager.get_current_device_id() {
                    None
                } else {
                    Some(info.target_device)
                }
            }
            None => None,
        };

        Ok(ret)
    }

    async fn get_processor(
        &self,
        target: Option<&ObjectId>,
    ) -> BuckyResult<CryptoInputProcessorRef> {
        if let Some(device_id) = self.get_target(target).await? {
            debug!("crypto target resolved: {:?} -> {}", target, device_id);
            let processor = self.get_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.processor.clone())
        }
    }
}

#[async_trait::async_trait]
impl CryptoInputProcessor for CryptoRouter {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.verify_object(req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.sign_object(req).await
    }
}
