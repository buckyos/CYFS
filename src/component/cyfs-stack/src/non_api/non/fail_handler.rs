use crate::meta::ObjectFailHandler;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NONOutputFailHandleProcessor {
    next: NONOutputProcessorRef,
    target: DeviceId,
    fail_handler: ObjectFailHandler,
}

impl NONOutputFailHandleProcessor {
    pub fn new(
        target: DeviceId,
        fail_handler: ObjectFailHandler,
        next: NONOutputProcessorRef,
    ) -> NONOutputProcessorRef {
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
impl NONOutputProcessor for NONOutputFailHandleProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        self.next.put_object(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn get_object(
        &self,
        req: NONGetObjectOutputRequest,
    ) -> BuckyResult<NONGetObjectOutputResponse> {
        self.next.get_object(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn post_object(
        &self,
        req: NONPostObjectOutputRequest,
    ) -> BuckyResult<NONPostObjectOutputResponse> {
        self.next.post_object(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn select_object(
        &self,
        req: NONSelectObjectOutputRequest,
    ) -> BuckyResult<NONSelectObjectOutputResponse> {
        self.next.select_object(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectOutputRequest,
    ) -> BuckyResult<NONDeleteObjectOutputResponse> {
        self.next.delete_object(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }
}
