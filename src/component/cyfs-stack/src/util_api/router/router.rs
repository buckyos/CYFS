use super::super::acl::UtilAclInnerInputProcessor;
use super::super::local::UtilLocalService;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::util::*;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct UtilRouter {
    processor: UtilInputProcessorRef,

    zone_manager: ZoneManagerRef,

    forward: ForwardProcessorManager,

    fail_handler: ObjectFailHandler,
}

impl UtilRouter {
    pub(crate) fn new(
        local_service: UtilLocalService,
        zone_manager: ZoneManagerRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
    ) -> Self {
        let processor = local_service.clone_processor();

        // 限定同zone
        let processor = UtilAclInnerInputProcessor::new(processor);

        Self {
            processor,

            zone_manager,
            forward,
            fail_handler,
        }
    }

    pub fn clone_processor(&self) -> UtilInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    async fn get_forward(&self, target: DeviceId) -> BuckyResult<UtilInputProcessorRef> {
        // 获取到目标的processor
        let requestor = self.forward.get(&target).await?;

        // TODO 权限
        let processor = UtilRequestor::new(None, requestor).into_processor();

        // 转换为input processor
        let input_processor = UtilInputTransformer::new(processor);

        Ok(input_processor)
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

    async fn get_processor(&self, target: Option<&ObjectId>) -> BuckyResult<UtilInputProcessorRef> {
        if let Some(device_id) = self.get_target(target).await? {
            debug!("util target resolved: {:?} -> {}", target, device_id);
            let processor = self.get_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.processor.clone())
        }
    }

    async fn get_device(
        &self,
        req: UtilGetDeviceInputRequest,
    ) -> BuckyResult<UtilGetDeviceInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_device(req).await
    }

    async fn get_zone(
        &self,
        req: UtilGetZoneInputRequest,
    ) -> BuckyResult<UtilGetZoneInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_zone(req).await
    }

    async fn resolve_ood(
        &self,
        req: UtilResolveOODInputRequest,
    ) -> BuckyResult<UtilResolveOODInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.resolve_ood(req).await
    }

    async fn get_ood_status(
        &self,
        req: UtilGetOODStatusInputRequest,
    ) -> BuckyResult<UtilGetOODStatusInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_ood_status(req).await
    }

    async fn get_noc_info(
        &self,
        req: UtilGetNOCInfoInputRequest,
    ) -> BuckyResult<UtilGetNOCInfoInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_noc_info(req).await
    }

    async fn get_network_access_info(
        &self,
        req: UtilGetNetworkAccessInfoInputRequest,
    ) -> BuckyResult<UtilGetNetworkAccessInfoInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_network_access_info(req).await
    }

    async fn get_device_static_info(
        &self,
        req: UtilGetDeviceStaticInfoInputRequest,
    ) -> BuckyResult<UtilGetDeviceStaticInfoInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_device_static_info(req).await
    }

    async fn get_system_info(
        &self,
        req: UtilGetSystemInfoInputRequest,
    ) -> BuckyResult<UtilGetSystemInfoInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_system_info(req).await
    }

    async fn get_version_info(
        &self,
        req: UtilGetVersionInfoInputRequest,
    ) -> BuckyResult<UtilGetVersionInfoInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.get_version_info(req).await
    }

    async fn build_file_object(
        &self,
        req: UtilBuildFileInputRequest,
    ) -> BuckyResult<UtilBuildFileInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.build_file_object(req).await
    }
}

#[async_trait::async_trait]
impl UtilInputProcessor for UtilRouter {
    async fn get_device(
        &self,
        req: UtilGetDeviceInputRequest,
    ) -> BuckyResult<UtilGetDeviceInputResponse> {
        Self::get_device(&self, req).await
    }

    async fn get_zone(
        &self,
        req: UtilGetZoneInputRequest,
    ) -> BuckyResult<UtilGetZoneInputResponse> {
        Self::get_zone(&self, req).await
    }

    async fn resolve_ood(
        &self,
        req: UtilResolveOODInputRequest,
    ) -> BuckyResult<UtilResolveOODInputResponse> {
        Self::resolve_ood(&self, req).await
    }

    async fn get_ood_status(
        &self,
        req: UtilGetOODStatusInputRequest,
    ) -> BuckyResult<UtilGetOODStatusInputResponse> {
        Self::get_ood_status(&self, req).await
    }

    async fn get_noc_info(
        &self,
        req: UtilGetNOCInfoInputRequest,
    ) -> BuckyResult<UtilGetNOCInfoInputResponse> {
        Self::get_noc_info(&self, req).await
    }

    async fn get_network_access_info(
        &self,
        req: UtilGetNetworkAccessInfoInputRequest,
    ) -> BuckyResult<UtilGetNetworkAccessInfoInputResponse> {
        Self::get_network_access_info(&self, req).await
    }

    async fn get_device_static_info(
        &self,
        req: UtilGetDeviceStaticInfoInputRequest,
    ) -> BuckyResult<UtilGetDeviceStaticInfoInputResponse> {
        Self::get_device_static_info(&self, req).await
    }

    async fn get_system_info(
        &self,
        req: UtilGetSystemInfoInputRequest,
    ) -> BuckyResult<UtilGetSystemInfoInputResponse> {
        Self::get_system_info(&self, req).await
    }

    async fn get_version_info(
        &self,
        req: UtilGetVersionInfoInputRequest,
    ) -> BuckyResult<UtilGetVersionInfoInputResponse> {
        Self::get_version_info(&self, req).await
    }

    async fn build_file_object(
        &self,
        req: UtilBuildFileInputRequest,
    ) -> BuckyResult<UtilBuildFileInputResponse> {
        Self::build_file_object(self, req).await
    }

    async fn build_dir_from_object_map(
        &self,
        req: UtilBuildDirFromObjectMapInputRequest,
    ) -> BuckyResult<UtilBuildDirFromObjectMapInputResponse> {
        let processor = self.get_processor(req.common.target.as_ref()).await?;
        processor.build_dir_from_object_map(req).await
    }
}
