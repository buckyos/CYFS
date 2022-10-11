use super::processor::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 实现从input到output的转换
pub(crate) struct CryptoInputTransformer {
    processor: CryptoOutputProcessorRef,
}

impl CryptoInputTransformer {
    pub fn new(processor: CryptoOutputProcessorRef) -> CryptoInputProcessorRef {
        let ret = Self { processor };
        Arc::new(Box::new(ret))
    }

    fn convert_common(common: CryptoInputRequestCommon) -> CryptoOutputRequestCommon {
        CryptoOutputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 来源DEC
            dec_id: common.source.get_opt_dec().cloned(),

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,
        }
    }

    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectOutputResponse> {
        let out_req = CryptoVerifyObjectOutputRequest {
            common: Self::convert_common(req.common),

            object: req.object,
            sign_type: req.sign_type,
            sign_object: req.sign_object,
        };

        let out_resp = self.processor.verify_object(out_req).await?;

        Ok(out_resp)
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        let out_req = CryptoSignObjectOutputRequest {
            common: Self::convert_common(req.common),

            object: req.object,
            flags: req.flags,
        };

        let out_resp = self.processor.sign_object(out_req).await?;

        Ok(out_resp)
    }
}

#[async_trait::async_trait]
impl CryptoInputProcessor for CryptoInputTransformer {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        CryptoInputTransformer::verify_object(&self, req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        CryptoInputTransformer::sign_object(&self, req).await
    }
}

// 实现从output到input的转换
pub(crate) struct CryptoOutputTransformer {
    processor: CryptoInputProcessorRef,
    source: RequestSourceInfo,
}

impl CryptoOutputTransformer {
    pub fn new(
        processor: CryptoInputProcessorRef,
        source: RequestSourceInfo,
    ) -> CryptoOutputProcessorRef {
        let ret = Self { processor, source };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: CryptoOutputRequestCommon) -> CryptoInputRequestCommon {
        let mut source = self.source.clone();
        if let Some(dec_id) = common.dec_id {
            source.set_dec(dec_id);
        }

        CryptoInputRequestCommon {
            // 请求路径，可为空
            req_path: common.req_path,

            // 用以处理默认行为
            target: common.target,

            flags: common.flags,

            source,
        }
    }

    async fn verify_object(
        &self,
        req: CryptoVerifyObjectOutputRequest,
    ) -> BuckyResult<CryptoVerifyObjectOutputResponse> {
        let in_req = CryptoVerifyObjectInputRequest {
            common: self.convert_common(req.common),

            object: req.object,

            sign_type: req.sign_type,
            sign_object: req.sign_object,
        };

        let resp = self.processor.verify_object(in_req).await?;

        Ok(resp)
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectOutputRequest,
    ) -> BuckyResult<CryptoSignObjectOutputResponse> {
        let in_req = CryptoSignObjectInputRequest {
            common: self.convert_common(req.common),

            object: req.object,
            flags: req.flags,
        };

        let resp = self.processor.sign_object(in_req).await?;

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl CryptoOutputProcessor for CryptoOutputTransformer {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectOutputRequest,
    ) -> BuckyResult<CryptoVerifyObjectOutputResponse> {
        CryptoOutputTransformer::verify_object(&self, req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectOutputRequest,
    ) -> BuckyResult<CryptoSignObjectOutputResponse> {
        CryptoOutputTransformer::sign_object(&self, req).await
    }
}
