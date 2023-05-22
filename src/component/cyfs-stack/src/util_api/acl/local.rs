use crate::util::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 限定在同zone内操作
pub(crate) struct UtilAclInnerInputProcessor {
    next: UtilInputProcessorRef,
}

impl UtilAclInnerInputProcessor {
    pub(crate) fn new(next: UtilInputProcessorRef) -> UtilInputProcessorRef {
        let ret = Self { next };

        Arc::new(Box::new(ret))
    }

    fn check_local_zone_permit(
        &self,
        service: &str,
        source: &RequestSourceInfo,
    ) -> BuckyResult<()> {
        if !source.is_current_zone() {
            let msg = format!(
                "{} service valid only in current zone! source={:?}, category={}",
                service,
                source.zone.device,
                source.zone.zone_category.as_str()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl UtilInputProcessor for UtilAclInnerInputProcessor {
    async fn get_device(
        &self,
        req: UtilGetDeviceInputRequest,
    ) -> BuckyResult<UtilGetDeviceInputResponse> {
        self.check_local_zone_permit("util.get_device", &req.common.source)?;

        self.next.get_device(req).await
    }

    async fn get_zone(
        &self,
        req: UtilGetZoneInputRequest,
    ) -> BuckyResult<UtilGetZoneInputResponse> {
        self.check_local_zone_permit("util.get_zone", &req.common.source)?;

        self.next.get_zone(req).await
    }

    async fn resolve_ood(
        &self,
        req: UtilResolveOODInputRequest,
    ) -> BuckyResult<UtilResolveOODInputResponse> {
        self.check_local_zone_permit("util.resolve_ood", &req.common.source)?;

        self.next.resolve_ood(req).await
    }

    async fn get_ood_status(
        &self,
        req: UtilGetOODStatusInputRequest,
    ) -> BuckyResult<UtilGetOODStatusInputResponse> {
        self.check_local_zone_permit("util.get_ood_status", &req.common.source)?;

        self.next.get_ood_status(req).await
    }

    async fn get_noc_info(
        &self,
        req: UtilGetNOCInfoInputRequest,
    ) -> BuckyResult<UtilGetNOCInfoInputResponse> {
        self.check_local_zone_permit("util.get_noc_info", &req.common.source)?;

        self.next.get_noc_info(req).await
    }

    async fn get_network_access_info(
        &self,
        req: UtilGetNetworkAccessInfoInputRequest,
    ) -> BuckyResult<UtilGetNetworkAccessInfoInputResponse> {
        self.check_local_zone_permit("util.get_network_access_info", &req.common.source)?;

        self.next.get_network_access_info(req).await
    }

    async fn get_device_static_info(
        &self,
        req: UtilGetDeviceStaticInfoInputRequest,
    ) -> BuckyResult<UtilGetDeviceStaticInfoInputResponse> {
        self.check_local_zone_permit("util.get_device_static_info", &req.common.source)?;

        self.next.get_device_static_info(req).await
    }

    async fn get_system_info(
        &self,
        req: UtilGetSystemInfoInputRequest,
    ) -> BuckyResult<UtilGetSystemInfoInputResponse> {
        self.check_local_zone_permit("util.get_system_info", &req.common.source)?;

        self.next.get_system_info(req).await
    }

    async fn update_system_info(
        &self,
        req: UtilUpdateSystemInfoInputRequest,
    ) -> BuckyResult<UtilUpdateSystemInfoInputResponse> {
        self.check_local_zone_permit("util.update_system_info", &req.common.source)?;

        if !req.common.source.is_system_dec() {
            let msg = format!("util.update_system_info only valid for system dec!");
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }
        
        self.next.update_system_info(req).await
    }

    async fn get_version_info(
        &self,
        req: UtilGetVersionInfoInputRequest,
    ) -> BuckyResult<UtilGetVersionInfoInputResponse> {
        self.check_local_zone_permit("util.get_version_info", &req.common.source)?;

        self.next.get_version_info(req).await
    }

    async fn build_file_object(
        &self,
        req: UtilBuildFileInputRequest,
    ) -> BuckyResult<UtilBuildFileInputResponse> {
        self.check_local_zone_permit("util.build_file_object", &req.common.source)?;

        self.next.build_file_object(req).await
    }

    async fn build_dir_from_object_map(
        &self,
        req: UtilBuildDirFromObjectMapInputRequest,
    ) -> BuckyResult<UtilBuildDirFromObjectMapInputResponse> {
        self.check_local_zone_permit("util.build_dir_from_object_map", &req.common.source)?;

        self.next.build_dir_from_object_map(req).await
    }
}
