use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 实现从input到output的转换
pub(crate) struct NONInputTransformer {
    processor: NONOutputProcessorRef,
}

impl NONInputTransformer {
    pub fn new(processor: NONOutputProcessorRef) -> NONInputProcessorRef {
        let ret = Self { processor };
        Arc::new(Box::new(ret))
    }

    fn convert_common(common: NONInputRequestCommon) -> NONOutputRequestCommon {
        NONOutputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 来源DEC
            dec_id: common.dec_id,

            // 目标DEC
            target_dec_id: common.target_dec_id,

            // 默认行为
            level: common.level,

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,
        }
    }

    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        let out_req = NONPutObjectOutputRequest {
            common: Self::convert_common(req.common),

            object: req.object,
        };

        let out_resp = self.processor.put_object(out_req).await?;

        let resp = NONPutObjectInputResponse {
            result: out_resp.result,
            object_expires_time: out_resp.object_expires_time,
            object_update_time: out_resp.object_update_time,
        };

        Ok(resp)
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let out_req = NONGetObjectOutputRequest {
            common: Self::convert_common(req.common),

            object_id: req.object_id,
            inner_path: req.inner_path,
        };

        let out_resp = self.processor.get_object(out_req).await?;

        let resp = NONGetObjectInputResponse {
            object: out_resp.object,
            object_expires_time: out_resp.object_expires_time,
            object_update_time: out_resp.object_update_time,
            attr: out_resp.attr,
        };

        Ok(resp)
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        let out_req = NONPostObjectOutputRequest {
            common: Self::convert_common(req.common),

            object: req.object,
        };

        let out_resp = self.processor.post_object(out_req).await?;

        let resp = NONPostObjectInputResponse {
            object: out_resp.object,
        };

        Ok(resp)
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        let out_req = NONSelectObjectOutputRequest {
            common: Self::convert_common(req.common),

            filter: req.filter,
            opt: req.opt,
        };

        let out_resp = self.processor.select_object(out_req).await?;

        let resp = NONSelectObjectInputResponse {
            objects: out_resp.objects,
        };

        Ok(resp)
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        let out_req = NONDeleteObjectOutputRequest {
            common: Self::convert_common(req.common),

            object_id: req.object_id,
            inner_path: req.inner_path,
        };

        let out_resp = self.processor.delete_object(out_req).await?;

        let resp = NONDeleteObjectInputResponse {
            object: out_resp.object,
        };

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONInputTransformer {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        NONInputTransformer::put_object(&self, req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        NONInputTransformer::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        NONInputTransformer::post_object(&self, req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        NONInputTransformer::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        NONInputTransformer::delete_object(&self, req).await
    }
}

// 实现从output到input的转换
pub(crate) struct NONOutputTransformer {
    processor: NONInputProcessorRef,
    source: DeviceId,
}

impl NONOutputTransformer {
    pub fn new(processor: NONInputProcessorRef, source: DeviceId) -> NONOutputProcessorRef {
        let ret = Self { processor, source };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: NONOutputRequestCommon) -> NONInputRequestCommon {
        NONInputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 来源DEC
            dec_id: common.dec_id,

            // 目标DEC
            target_dec_id: common.target_dec_id,

            // 默认行为
            level: common.level,

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,

            source: self.source.clone(),
            protocol: NONProtocol::Native,
        }
    }

    async fn put_object(
        &self,
        req: NONPutObjectOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        let in_req = NONPutObjectInputRequest {
            common: self.convert_common(req.common),

            object: req.object,
        };

        let in_resp = self.processor.put_object(in_req).await?;

        let resp = NONPutObjectOutputResponse {
            result: in_resp.result,
            object_expires_time: in_resp.object_expires_time,
            object_update_time: in_resp.object_update_time,
        };

        Ok(resp)
    }

    async fn get_object(
        &self,
        req: NONGetObjectOutputRequest,
    ) -> BuckyResult<NONGetObjectOutputResponse> {
        let in_req = NONGetObjectInputRequest {
            common: self.convert_common(req.common),

            object_id: req.object_id,
            inner_path: req.inner_path,
        };

        let in_resp = self.processor.get_object(in_req).await?;

        let resp = NONGetObjectOutputResponse {
            object: in_resp.object,
            object_expires_time: in_resp.object_expires_time,
            object_update_time: in_resp.object_update_time,
            attr: in_resp.attr,
        };

        Ok(resp)
    }

    async fn post_object(
        &self,
        req: NONPostObjectOutputRequest,
    ) -> BuckyResult<NONPostObjectOutputResponse> {
        let in_req = NONPostObjectInputRequest {
            common: self.convert_common(req.common),

            object: req.object,
        };

        let in_resp = self.processor.post_object(in_req).await?;

        let resp = NONPostObjectOutputResponse {
            object: in_resp.object,
        };

        Ok(resp)
    }

    async fn select_object(
        &self,
        req: NONSelectObjectOutputRequest,
    ) -> BuckyResult<NONSelectObjectOutputResponse> {
        let in_req = NONSelectObjectInputRequest {
            common: self.convert_common(req.common),

            filter: req.filter,
            opt: req.opt,
        };

        let in_resp = self.processor.select_object(in_req).await?;

        let resp = NONSelectObjectOutputResponse {
            objects: in_resp.objects,
        };

        Ok(resp)
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectOutputRequest,
    ) -> BuckyResult<NONDeleteObjectOutputResponse> {
        let in_req = NONDeleteObjectInputRequest {
            common: self.convert_common(req.common),

            object_id: req.object_id,
            inner_path: req.inner_path,
        };

        let in_resp = self.processor.delete_object(in_req).await?;

        let resp = NONDeleteObjectOutputResponse {
            object: in_resp.object,
        };

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl NONOutputProcessor for NONOutputTransformer {
    async fn put_object(
        &self,
        req: NONPutObjectOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        NONOutputTransformer::put_object(&self, req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectOutputRequest,
    ) -> BuckyResult<NONGetObjectOutputResponse> {
        NONOutputTransformer::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectOutputRequest,
    ) -> BuckyResult<NONPostObjectOutputResponse> {
        NONOutputTransformer::post_object(&self, req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectOutputRequest,
    ) -> BuckyResult<NONSelectObjectOutputResponse> {
        NONOutputTransformer::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectOutputRequest,
    ) -> BuckyResult<NONDeleteObjectOutputResponse> {
        NONOutputTransformer::delete_object(&self, req).await
    }
}
