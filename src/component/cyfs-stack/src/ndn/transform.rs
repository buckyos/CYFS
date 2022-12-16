use crate::ndn::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 实现从input到output的转换
pub(crate) struct NDNInputTransformer {
    processor: NDNOutputProcessorRef,
}

impl NDNInputTransformer {
    pub fn new(processor: NDNOutputProcessorRef) -> NDNInputProcessorRef {
        let ret = Self { processor };
        Arc::new(Box::new(ret))
    }

    fn convert_common(common: NDNInputRequestCommon) -> NDNOutputRequestCommon {
        NDNOutputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 来源DEC
            dec_id: common.source.get_opt_dec().cloned(),

            // 默认行为
            level: common.level,

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,

            referer_object: common.referer_object,
        }
    }

    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        let out_req = NDNPutDataOutputRequest {
            common: Self::convert_common(req.common),

            object_id: req.object_id,
            length: req.length,
            data: req.data,
        };

        let out_resp = self.processor.put_data(out_req).await?;

        let resp = NDNPutDataInputResponse {
            result: out_resp.result,
        };

        Ok(resp)
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        let out_req = NDNGetDataOutputRequest {
            common: Self::convert_common(req.common),

            object_id: req.object_id,
            range: req.range,
            inner_path: req.inner_path,
            group: req.group,
        };

        let out_resp = self.processor.get_data(out_req).await?;

        let resp = NDNGetDataInputResponse {
            object_id: out_resp.object_id,
            owner_id: out_resp.owner_id,
            attr: out_resp.attr,
            length: out_resp.length,
            range: out_resp.range,
            data: out_resp.data,
        };

        Ok(resp)
    }

    async fn put_shared_data(
        &self,
        req: NDNPutDataInputRequest,
    ) -> BuckyResult<NDNPutDataInputResponse> {
        let out_req = NDNPutDataOutputRequest {
            common: Self::convert_common(req.common),

            object_id: req.object_id,
            length: req.length,
            data: req.data,
        };

        let out_resp = self.processor.put_shared_data(out_req).await?;

        let resp = NDNPutDataInputResponse {
            result: out_resp.result,
        };

        Ok(resp)
    }

    async fn get_shared_data(
        &self,
        req: NDNGetDataInputRequest,
    ) -> BuckyResult<NDNGetDataInputResponse> {
        let out_req = NDNGetDataOutputRequest {
            common: Self::convert_common(req.common),

            object_id: req.object_id,
            range: req.range,
            inner_path: req.inner_path,
            group: req.group,
        };

        let out_resp = self.processor.get_shared_data(out_req).await?;

        let resp = NDNGetDataInputResponse {
            object_id: out_resp.object_id,
            owner_id: out_resp.owner_id,
            attr: out_resp.attr,
            length: out_resp.length,
            range: out_resp.range,
            data: out_resp.data,
        };

        Ok(resp)
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        let out_req = NDNDeleteDataOutputRequest {
            common: Self::convert_common(req.common),

            object_id: req.object_id,
            inner_path: req.inner_path,
        };

        let out_resp = self.processor.delete_data(out_req).await?;

        let resp = NDNDeleteDataInputResponse {
            object_id: out_resp.object_id,
        };

        Ok(resp)
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        let out_req = NDNQueryFileOutputRequest {
            common: Self::convert_common(req.common),

            param: req.param,
        };

        let out_resp = self.processor.query_file(out_req).await?;

        Ok(out_resp)
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNInputTransformer {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        NDNInputTransformer::put_data(&self, req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        NDNInputTransformer::get_data(&self, req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        NDNInputTransformer::delete_data(&self, req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        NDNInputTransformer::query_file(&self, req).await
    }
}

// 实现从output到input的转换
pub(crate) struct NDNOutputTransformer {
    processor: NDNInputProcessorRef,
    source: RequestSourceInfo,
}

impl NDNOutputTransformer {
    pub fn new(
        processor: NDNInputProcessorRef,
        source: RequestSourceInfo,
    ) -> NDNOutputProcessorRef {
        let ret = Self { processor, source };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: NDNOutputRequestCommon) -> NDNInputRequestCommon {
        let mut source = self.source.clone();
        if let Some(dec_id) = common.dec_id {
            source.set_dec(dec_id);
        }

        NDNInputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 默认行为
            level: common.level,

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,

            referer_object: common.referer_object,

            source,
            user_data: None,
        }
    }

    async fn put_data(
        &self,
        req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        let in_req = NDNPutDataInputRequest {
            common: self.convert_common(req.common),

            object_id: req.object_id,
            data_type: NDNDataType::Mem,
            length: req.length,
            data: req.data,
        };

        let in_resp = self.processor.put_data(in_req).await?;

        let resp = NDNPutDataOutputResponse {
            result: in_resp.result,
        };

        Ok(resp)
    }

    async fn get_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        let in_req = NDNGetDataInputRequest {
            common: self.convert_common(req.common),

            object_id: req.object_id,
            data_type: NDNDataType::Mem,
            range: req.range,
            inner_path: req.inner_path,
            group: req.group,
        };

        let in_resp = self.processor.get_data(in_req).await?;

        let resp = NDNGetDataOutputResponse {
            object_id: in_resp.object_id,
            owner_id: in_resp.owner_id,
            attr: in_resp.attr,
            range: in_resp.range,
            length: in_resp.length,
            data: in_resp.data,
        };

        Ok(resp)
    }

    async fn put_shared_data(
        &self,
        req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        let in_req = NDNPutDataInputRequest {
            common: self.convert_common(req.common),

            object_id: req.object_id,
            data_type: NDNDataType::SharedMem,
            length: req.length,
            data: req.data,
        };

        let in_resp = self.processor.put_data(in_req).await?;

        let resp = NDNPutDataOutputResponse {
            result: in_resp.result,
        };

        Ok(resp)
    }

    async fn get_shared_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        let in_req = NDNGetDataInputRequest {
            common: self.convert_common(req.common),

            object_id: req.object_id,
            data_type: NDNDataType::SharedMem,
            range: req.range,
            inner_path: req.inner_path,
            group: req.group,
        };

        let in_resp = self.processor.get_data(in_req).await?;

        let resp = NDNGetDataOutputResponse {
            object_id: in_resp.object_id,
            owner_id: in_resp.owner_id,
            attr: in_resp.attr,
            range: in_resp.range,
            length: in_resp.length,
            data: in_resp.data,
        };

        Ok(resp)
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataOutputRequest,
    ) -> BuckyResult<NDNDeleteDataOutputResponse> {
        let in_req = NDNDeleteDataInputRequest {
            common: self.convert_common(req.common),

            object_id: req.object_id,
            inner_path: req.inner_path,
        };

        let in_resp = self.processor.delete_data(in_req).await?;

        let resp = NDNDeleteDataOutputResponse {
            object_id: in_resp.object_id,
        };

        Ok(resp)
    }

    async fn query_file(
        &self,
        req: NDNQueryFileOutputRequest,
    ) -> BuckyResult<NDNQueryFileOutputResponse> {
        let in_req = NDNQueryFileInputRequest {
            common: self.convert_common(req.common),

            param: req.param,
        };

        let resp = self.processor.query_file(in_req).await?;

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl NDNOutputProcessor for NDNOutputTransformer {
    async fn put_data(
        &self,
        req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        NDNOutputTransformer::put_data(&self, req).await
    }

    async fn get_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        NDNOutputTransformer::get_data(&self, req).await
    }

    async fn put_shared_data(
        &self,
        req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        NDNOutputTransformer::put_shared_data(&self, req).await
    }

    async fn get_shared_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        NDNOutputTransformer::get_shared_data(&self, req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataOutputRequest,
    ) -> BuckyResult<NDNDeleteDataOutputResponse> {
        NDNOutputTransformer::delete_data(&self, req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileOutputRequest,
    ) -> BuckyResult<NDNQueryFileOutputResponse> {
        NDNOutputTransformer::query_file(&self, req).await
    }
}
