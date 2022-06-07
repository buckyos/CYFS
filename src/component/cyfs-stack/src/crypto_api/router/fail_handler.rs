use crate::meta::ObjectFailHandler;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct CryptoOutputFailHandleProcessor {
    next: CryptoOutputProcessorRef,
    target: DeviceId,
    fail_handler: ObjectFailHandler,
}

impl CryptoOutputFailHandleProcessor {
    pub fn new(
        target: DeviceId,
        fail_handler: ObjectFailHandler,
        next: CryptoOutputProcessorRef,
    ) -> CryptoOutputProcessorRef {
        let ret = Self {
            next,
            target,
            fail_handler,
        };

        Arc::new(Box::new(ret))
    }

    fn on_connect_failed(&self, e: &BuckyError) {
        if e.code() == BuckyErrorCode::ConnectFailed {
            self.fail_handler.on_device_fail(&self.target);
        }
    }
}

#[async_trait::async_trait]
impl CryptoOutputProcessor for CryptoOutputFailHandleProcessor {
    async fn sign_object(
        &self,
        req: CryptoSignObjectOutputRequest,
    ) -> BuckyResult<CryptoSignObjectOutputResponse> {
        self.next.sign_object(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn verify_object(
        &self,
        req: CryptoVerifyObjectOutputRequest,
    ) -> BuckyResult<CryptoVerifyObjectOutputResponse> {
        self.next.verify_object(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }
}
