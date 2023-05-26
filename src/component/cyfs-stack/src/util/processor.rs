use cyfs_lib::*;
use cyfs_base::*;

use std::sync::Arc;



#[async_trait::async_trait]
pub trait UtilInputProcessor: Sync + Send + 'static {
    async fn get_device(
        &self,
        req: UtilGetDeviceInputRequest,
    ) -> BuckyResult<UtilGetDeviceInputResponse>;

    async fn get_zone(&self, req: UtilGetZoneInputRequest)
        -> BuckyResult<UtilGetZoneInputResponse>;

    async fn resolve_ood(&self, req: UtilResolveOODInputRequest)
        -> BuckyResult<UtilResolveOODInputResponse>;

    async fn get_ood_status(&self, req: UtilGetOODStatusInputRequest)
        -> BuckyResult<UtilGetOODStatusInputResponse>;

    async fn get_noc_info(&self, req: UtilGetNOCInfoInputRequest)
        -> BuckyResult<UtilGetNOCInfoInputResponse>;

    async fn get_network_access_info(&self, req: UtilGetNetworkAccessInfoInputRequest)
        -> BuckyResult<UtilGetNetworkAccessInfoInputResponse>;

    async fn get_device_static_info(&self, req: UtilGetDeviceStaticInfoInputRequest)
        -> BuckyResult<UtilGetDeviceStaticInfoInputResponse>;

    async fn get_system_info(&self, req: UtilGetSystemInfoInputRequest)
        -> BuckyResult<UtilGetSystemInfoInputResponse>;
    async fn update_system_info(&self, req: UtilUpdateSystemInfoInputRequest)
        -> BuckyResult<UtilUpdateSystemInfoInputResponse>;

    async fn get_version_info(&self, req: UtilGetVersionInfoInputRequest)
        -> BuckyResult<UtilGetVersionInfoInputResponse>;

    async fn build_file_object(&self, req: UtilBuildFileInputRequest)
        -> BuckyResult<UtilBuildFileInputResponse>;

    async fn build_dir_from_object_map(&self, req: UtilBuildDirFromObjectMapInputRequest)
        -> BuckyResult<UtilBuildDirFromObjectMapInputResponse>;
}

pub type UtilInputProcessorRef = Arc<Box<dyn UtilInputProcessor>>;

