use crate::acl::*;
use crate::util::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 限定在同zone内操作
pub(crate) struct UtilAclInnerInputProcessor {
    acl: AclManagerRef,
    next: UtilInputProcessorRef,
}

impl UtilAclInnerInputProcessor {
    pub(crate) fn new(acl: AclManagerRef, next: UtilInputProcessorRef) -> UtilInputProcessorRef {
        let ret = Self { acl, next };

        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl UtilInputProcessor for UtilAclInnerInputProcessor {
    async fn get_device(
        &self,
        req: UtilGetDeviceInputRequest,
    ) -> BuckyResult<UtilGetDeviceInputResponse> {
        self.acl
            .check_local_zone_permit("util in get_device", &req.common.source)
            .await?;

        self.next.get_device(req).await
    }

    async fn get_zone(
        &self,
        req: UtilGetZoneInputRequest,
    ) -> BuckyResult<UtilGetZoneInputResponse> {
        self.acl
            .check_local_zone_permit("util in get_zone", &req.common.source)
            .await?;

        self.next.get_zone(req).await
    }

    async fn resolve_ood(
        &self,
        req: UtilResolveOODInputRequest,
    ) -> BuckyResult<UtilResolveOODInputResponse> {
        self.acl
            .check_local_zone_permit("util in resolve_ood", &req.common.source)
            .await?;

        self.next.resolve_ood(req).await
    }

    async fn get_ood_status(
        &self,
        req: UtilGetOODStatusInputRequest,
    ) -> BuckyResult<UtilGetOODStatusInputResponse> {
        self.acl
            .check_local_zone_permit("util in get_ood_status", &req.common.source)
            .await?;

        self.next.get_ood_status(req).await
    }

    async fn get_noc_info(
        &self,
        req: UtilGetNOCInfoInputRequest,
    ) -> BuckyResult<UtilGetNOCInfoInputResponse> {
        self.acl
            .check_local_zone_permit("util in get_noc_info", &req.common.source)
            .await?;

        self.next.get_noc_info(req).await
    }

    async fn get_network_access_info(
        &self,
        req: UtilGetNetworkAccessInfoInputRequest,
    ) -> BuckyResult<UtilGetNetworkAccessInfoInputResponse> {
        self.acl
            .check_local_zone_permit("util in get_network_access_info", &req.common.source)
            .await?;

        self.next.get_network_access_info(req).await
    }

    async fn get_device_static_info(
        &self,
        req: UtilGetDeviceStaticInfoInputRequest,
    ) -> BuckyResult<UtilGetDeviceStaticInfoInputResponse> {
        self.acl
            .check_local_zone_permit("util in get_device_static_info", &req.common.source)
            .await?;

        self.next.get_device_static_info(req).await
    }

    async fn get_system_info(
        &self,
        req: UtilGetSystemInfoInputRequest,
    ) -> BuckyResult<UtilGetSystemInfoInputResponse> {
        self.acl
            .check_local_zone_permit("util in get_system_info", &req.common.source)
            .await?;

        self.next.get_system_info(req).await
    }

    async fn get_version_info(
        &self,
        req: UtilGetVersionInfoInputRequest,
    ) -> BuckyResult<UtilGetVersionInfoInputResponse> {
        self.acl
            .check_local_zone_permit("util in get_version_info", &req.common.source)
            .await?;

        self.next.get_version_info(req).await
    }

    async fn build_file_object(&self, req: UtilBuildFileInputRequest) -> BuckyResult<UtilBuildFileInputResponse> {
        self.acl
            .check_local_zone_permit("util in build_file_object", &req.common.source)
            .await?;

        self.next.build_file_object(req).await
    }

    async fn build_dir_from_object_map(&self, req: UtilBuildDirFromObjectMapInputRequest)
        -> BuckyResult<UtilBuildDirFromObjectMapInputResponse> {
        self.acl
            .check_local_zone_permit("util in build_dir_from_object_map", &req.common.source)
            .await?;

        self.next.build_dir_from_object_map(req).await
    }
}
