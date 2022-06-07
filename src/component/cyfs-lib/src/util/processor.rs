use super::output_request::*;
use cyfs_base::*;

use std::sync::Arc;



#[async_trait::async_trait]
pub trait UtilOutputProcessor: Sync + Send + 'static {
    async fn get_device(
        &self,
        req: UtilGetDeviceOutputRequest,
    ) -> BuckyResult<UtilGetDeviceOutputResponse>;

    async fn get_zone(&self, req: UtilGetZoneOutputRequest)
        -> BuckyResult<UtilGetZoneOutputResponse>;

    async fn resolve_ood(&self, req: UtilResolveOODOutputRequest)
        -> BuckyResult<UtilResolveOODOutputResponse>;

    async fn get_ood_status(&self, req: UtilGetOODStatusOutputRequest)
        -> BuckyResult<UtilGetOODStatusOutputResponse>;

    async fn get_noc_info(&self, req: UtilGetNOCInfoOutputRequest)
        -> BuckyResult<UtilGetNOCInfoOutputResponse>;

    async fn get_network_access_info(&self, req: UtilGetNetworkAccessInfoOutputRequest)
        -> BuckyResult<UtilGetNetworkAccessInfoOutputResponse>;

    async fn get_device_static_info(&self, req: UtilGetDeviceStaticInfoOutputRequest)
        -> BuckyResult<UtilGetDeviceStaticInfoOutputResponse>;

    async fn get_system_info(&self, req: UtilGetSystemInfoOutputRequest)
        -> BuckyResult<UtilGetSystemInfoOutputResponse>;

    async fn get_version_info(&self, req: UtilGetVersionInfoOutputRequest)
        -> BuckyResult<UtilGetVersionInfoOutputResponse>;

    async fn build_file_object(&self, req: UtilBuildFileOutputRequest)
        -> BuckyResult<UtilBuildFileOutputResponse>;

    async fn build_dir_from_object_map(&self, req: UtilBuildDirFromObjectMapOutputRequest)
                                       -> BuckyResult<UtilBuildDirFromObjectMapOutputResponse>;
}

pub type UtilOutputProcessorRef = Arc<Box<dyn UtilOutputProcessor>>;

