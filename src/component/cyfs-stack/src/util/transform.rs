use super::processor::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 实现从input到output的转换
pub(crate) struct UtilInputTransformer {
    processor: UtilOutputProcessorRef,
}

impl UtilInputTransformer {
    pub fn new(processor: UtilOutputProcessorRef) -> UtilInputProcessorRef {
        let ret = Self { processor };
        Arc::new(Box::new(ret))
    }

    fn convert_common(common: UtilInputRequestCommon) -> UtilOutputRequestCommon {
        UtilOutputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 来源DEC
            dec_id: common.source.get_opt_dec().cloned(),

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,
        }
    }

    async fn get_device(
        &self,
        req: UtilGetDeviceInputRequest,
    ) -> BuckyResult<UtilGetDeviceInputResponse> {
        let out_req = UtilGetDeviceOutputRequest {
            common: Self::convert_common(req.common),
        };

        let out_resp = self.processor.get_device(out_req).await?;

        let resp = UtilGetDeviceInputResponse {
            device_id: out_resp.device_id,
            device: out_resp.device,
        };

        Ok(resp)
    }

    async fn get_zone(
        &self,
        req: UtilGetZoneInputRequest,
    ) -> BuckyResult<UtilGetZoneInputResponse> {
        let out_req = UtilGetZoneOutputRequest {
            common: Self::convert_common(req.common),
            object_id: req.object_id,
            object_raw: req.object_raw,
        };

        let out_resp = self.processor.get_zone(out_req).await?;

        let resp = UtilGetZoneInputResponse {
            zone_id: out_resp.zone_id,
            zone: out_resp.zone,
            device_id: out_resp.device_id,
        };

        Ok(resp)
    }

    async fn resolve_ood(
        &self,
        req: UtilResolveOODInputRequest,
    ) -> BuckyResult<UtilResolveOODInputResponse> {
        let out_req = UtilResolveOODOutputRequest {
            common: Self::convert_common(req.common),
            owner_id: req.owner_id,
            object_id: req.object_id,
        };

        let out_resp = self.processor.resolve_ood(out_req).await?;

        let resp = UtilResolveOODInputResponse {
            device_list: out_resp.device_list,
        };

        Ok(resp)
    }

    async fn get_ood_status(
        &self,
        req: UtilGetOODStatusInputRequest,
    ) -> BuckyResult<UtilGetOODStatusInputResponse> {
        let out_req = UtilGetOODStatusOutputRequest {
            common: Self::convert_common(req.common),
        };

        let out_resp = self.processor.get_ood_status(out_req).await?;

        let resp = UtilGetOODStatusInputResponse {
            status: out_resp.status,
        };

        Ok(resp)
    }

    async fn get_noc_info(
        &self,
        req: UtilGetNOCInfoInputRequest,
    ) -> BuckyResult<UtilGetNOCInfoInputResponse> {
        let out_req = UtilGetNOCInfoOutputRequest {
            common: Self::convert_common(req.common),
        };

        let out_resp = self.processor.get_noc_info(out_req).await?;

        let resp = UtilGetNOCInfoInputResponse {
            stat: out_resp.stat,
        };

        Ok(resp)
    }

    async fn get_network_access_info(
        &self,
        req: UtilGetNetworkAccessInfoInputRequest,
    ) -> BuckyResult<UtilGetNetworkAccessInfoInputResponse> {
        let out_req = UtilGetNetworkAccessInfoOutputRequest {
            common: Self::convert_common(req.common),
        };

        let out_resp = self.processor.get_network_access_info(out_req).await?;

        let resp = UtilGetNetworkAccessInfoInputResponse {
            info: out_resp.info,
        };

        Ok(resp)
    }

    async fn get_device_static_info(
        &self,
        req: UtilGetDeviceStaticInfoInputRequest,
    ) -> BuckyResult<UtilGetDeviceStaticInfoInputResponse> {
        let out_req = UtilGetDeviceStaticInfoOutputRequest {
            common: Self::convert_common(req.common),
        };

        let out_resp = self.processor.get_device_static_info(out_req).await?;

        let resp = UtilGetDeviceStaticInfoInputResponse {
            info: out_resp.info,
        };

        Ok(resp)
    }

    async fn get_system_info(
        &self,
        req: UtilGetSystemInfoInputRequest,
    ) -> BuckyResult<UtilGetSystemInfoInputResponse> {
        let out_req = UtilGetSystemInfoOutputRequest {
            common: Self::convert_common(req.common),
        };

        let out_resp = self.processor.get_system_info(out_req).await?;

        let resp = UtilGetSystemInfoInputResponse {
            info: out_resp.info,
        };

        Ok(resp)
    }

    async fn get_version_info(
        &self,
        req: UtilGetVersionInfoInputRequest,
    ) -> BuckyResult<UtilGetVersionInfoInputResponse> {
        let out_req = UtilGetVersionInfoOutputRequest {
            common: Self::convert_common(req.common),
        };

        let out_resp = self.processor.get_version_info(out_req).await?;

        let resp = UtilGetVersionInfoInputResponse {
            info: out_resp.info,
        };

        Ok(resp)
    }

    async fn build_file_object(
        &self,
        req: UtilBuildFileInputRequest
    ) -> BuckyResult<UtilBuildFileInputResponse> {
        let out_req = UtilBuildFileOutputRequest {
            common: Self::convert_common(req.common),
            local_path: req.local_path,
            owner: req.owner,
            chunk_size: req.chunk_size
        };

        let out_resp = self.processor.build_file_object(out_req).await?;
        Ok(out_resp)
    }

    async fn build_dir_from_object_map(
        &self,
        req: UtilBuildDirFromObjectMapInputRequest
    ) -> BuckyResult<UtilBuildDirFromObjectMapInputResponse> {
        let out_req = UtilBuildDirFromObjectMapOutputRequest {
            common: Self::convert_common(req.common),
            object_map_id: req.object_map_id,
            dir_type: req.dir_type,
        };

        let out_resp = self.processor.build_dir_from_object_map(out_req).await?;
        Ok(out_resp)
    }
}

#[async_trait::async_trait]
impl UtilInputProcessor for UtilInputTransformer {
    async fn get_device(
        &self,
        req: UtilGetDeviceInputRequest,
    ) -> BuckyResult<UtilGetDeviceInputResponse> {
        UtilInputTransformer::get_device(&self, req).await
    }

    async fn get_zone(
        &self,
        req: UtilGetZoneInputRequest,
    ) -> BuckyResult<UtilGetZoneInputResponse> {
        UtilInputTransformer::get_zone(&self, req).await
    }

    async fn resolve_ood(
        &self,
        req: UtilResolveOODInputRequest,
    ) -> BuckyResult<UtilResolveOODInputResponse> {
        UtilInputTransformer::resolve_ood(&self, req).await
    }

    async fn get_ood_status(
        &self,
        req: UtilGetOODStatusInputRequest,
    ) -> BuckyResult<UtilGetOODStatusInputResponse> {
        UtilInputTransformer::get_ood_status(&self, req).await
    }

    async fn get_noc_info(
        &self,
        req: UtilGetNOCInfoInputRequest,
    ) -> BuckyResult<UtilGetNOCInfoInputResponse> {
        UtilInputTransformer::get_noc_info(&self, req).await
    }

    async fn get_network_access_info(
        &self,
        req: UtilGetNetworkAccessInfoInputRequest,
    ) -> BuckyResult<UtilGetNetworkAccessInfoInputResponse> {
        UtilInputTransformer::get_network_access_info(&self, req).await
    }

    async fn get_device_static_info(
        &self,
        req: UtilGetDeviceStaticInfoInputRequest,
    ) -> BuckyResult<UtilGetDeviceStaticInfoInputResponse> {
        UtilInputTransformer::get_device_static_info(&self, req).await
    }

    async fn get_system_info(
        &self,
        req: UtilGetSystemInfoInputRequest,
    ) -> BuckyResult<UtilGetSystemInfoInputResponse> {
        UtilInputTransformer::get_system_info(&self, req).await
    }

    async fn get_version_info(
        &self,
        req: UtilGetVersionInfoInputRequest,
    ) -> BuckyResult<UtilGetVersionInfoInputResponse> {
        UtilInputTransformer::get_version_info(&self, req).await
    }

    async fn build_file_object(&self, req: UtilBuildFileInputRequest) -> BuckyResult<UtilBuildFileInputResponse> {
        UtilInputTransformer::build_file_object(&self, req).await
    }

    async fn build_dir_from_object_map(&self, req: UtilBuildDirFromObjectMapInputRequest)
        -> BuckyResult<UtilBuildDirFromObjectMapInputResponse> {
        UtilInputTransformer::build_dir_from_object_map(&self, req).await
    }
}

pub(crate) struct UtilOutputTransformer {
    processor: UtilInputProcessorRef,
    source: RequestSourceInfo,
}

impl UtilOutputTransformer {
    pub fn new(processor: UtilInputProcessorRef, source: RequestSourceInfo) -> UtilOutputProcessorRef {
        let ret = Self { processor, source };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: UtilOutputRequestCommon) -> UtilInputRequestCommon {
        let mut source = self.source.clone();
        source.set_dec(common.dec_id);

        UtilInputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,

            source,
        }
    }
}

#[async_trait::async_trait]
impl UtilOutputProcessor for UtilOutputTransformer {
    async fn get_device(
        &self,
        req: UtilGetDeviceOutputRequest,
    ) -> BuckyResult<UtilGetDeviceOutputResponse> {
        let in_req = UtilGetDeviceInputRequest {
            common: self.convert_common(req.common),
        };

        let resp = self.processor.get_device(in_req).await?;

        Ok(resp)
    }

    async fn get_zone(
        &self,
        req: UtilGetZoneOutputRequest,
    ) -> BuckyResult<UtilGetZoneOutputResponse> {
        let in_req = UtilGetZoneInputRequest {
            common: self.convert_common(req.common),
            object_id: req.object_id,
            object_raw: req.object_raw,
        };

        let resp = self.processor.get_zone(in_req).await?;

        Ok(resp)
    }

    async fn resolve_ood(
        &self,
        req: UtilResolveOODOutputRequest,
    ) -> BuckyResult<UtilResolveOODOutputResponse> {
        let in_req = UtilResolveOODInputRequest {
            common: self.convert_common(req.common),
            object_id: req.object_id,
            owner_id: req.owner_id,
        };

        let resp = self.processor.resolve_ood(in_req).await?;

        Ok(resp)
    }

    async fn get_ood_status(
        &self,
        req: UtilGetOODStatusOutputRequest,
    ) -> BuckyResult<UtilGetOODStatusOutputResponse> {
        let in_req = UtilGetOODStatusInputRequest {
            common: self.convert_common(req.common),
        };

        let resp = self.processor.get_ood_status(in_req).await?;

        Ok(resp)
    }

    async fn get_noc_info(
        &self,
        req: UtilGetNOCInfoOutputRequest,
    ) -> BuckyResult<UtilGetNOCInfoOutputResponse> {
        let in_req = UtilGetNOCInfoInputRequest {
            common: self.convert_common(req.common),
        };

        let resp = self.processor.get_noc_info(in_req).await?;

        Ok(resp)
    }

    async fn get_network_access_info(
        &self,
        req: UtilGetNetworkAccessInfoOutputRequest,
    ) -> BuckyResult<UtilGetNetworkAccessInfoOutputResponse> {
        let in_req = UtilGetNetworkAccessInfoInputRequest {
            common: self.convert_common(req.common),
        };

        let resp = self.processor.get_network_access_info(in_req).await?;

        Ok(resp)
    }

    async fn get_device_static_info(
        &self,
        req: UtilGetDeviceStaticInfoOutputRequest,
    ) -> BuckyResult<UtilGetDeviceStaticInfoOutputResponse> {
        let in_req = UtilGetDeviceStaticInfoInputRequest {
            common: self.convert_common(req.common),
        };

        let resp = self.processor.get_device_static_info(in_req).await?;

        Ok(resp)
    }

    async fn get_system_info(
        &self,
        req: UtilGetSystemInfoOutputRequest,
    ) -> BuckyResult<UtilGetSystemInfoOutputResponse> {
        let in_req = UtilGetSystemInfoInputRequest {
            common: self.convert_common(req.common),
        };

        let resp = self.processor.get_system_info(in_req).await?;

        Ok(resp)
    }

    async fn get_version_info(
        &self,
        req: UtilGetVersionInfoOutputRequest,
    ) -> BuckyResult<UtilGetVersionInfoOutputResponse> {
        let in_req = UtilGetVersionInfoInputRequest {
            common: self.convert_common(req.common),
        };

        let resp = self.processor.get_version_info(in_req).await?;

        Ok(resp)
    }

    async fn build_file_object(&self, req: UtilBuildFileOutputRequest) -> BuckyResult<UtilBuildFileOutputResponse> {
        let in_req = UtilBuildFileInputRequest {
            common: self.convert_common(req.common),
            local_path: req.local_path,
            owner: req.owner,
            chunk_size: req.chunk_size,
        };

        let resp = self.processor.build_file_object(in_req).await?;
        Ok(resp)
    }

    async fn build_dir_from_object_map(&self, req: UtilBuildDirFromObjectMapOutputRequest) -> BuckyResult<UtilBuildDirFromObjectMapOutputResponse> {
        let in_req = UtilBuildDirFromObjectMapInputRequest {
            common: self.convert_common(req.common),
            object_map_id: req.object_map_id,
            dir_type: req.dir_type,
        };
        let resp = self.processor.build_dir_from_object_map(in_req).await?;
        Ok(resp)
    }
}
