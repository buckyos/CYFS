use crate::meta::ObjectFailHandler;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NDNOutputFailHandleProcessor {
    next: NDNOutputProcessorRef,
    target: DeviceId,
    fail_handler: ObjectFailHandler,
}

impl NDNOutputFailHandleProcessor {
    pub fn new(
        target: DeviceId,
        fail_handler: ObjectFailHandler,
        next: NDNOutputProcessorRef,
    ) -> NDNOutputProcessorRef {
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
impl NDNOutputProcessor for NDNOutputFailHandleProcessor {
    async fn put_data(
        &self,
        req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        self.next.put_data(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn get_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        self.next.get_data(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn put_shared_data(
        &self,
        req: NDNPutDataOutputRequest,
    ) -> BuckyResult<NDNPutDataOutputResponse> {
        self.next.put_shared_data(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn get_shared_data(
        &self,
        req: NDNGetDataOutputRequest,
    ) -> BuckyResult<NDNGetDataOutputResponse> {
        self.next.get_shared_data(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataOutputRequest,
    ) -> BuckyResult<NDNDeleteDataOutputResponse> {
        self.next.delete_data(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }

    async fn query_file(
        &self,
        req: NDNQueryFileOutputRequest,
    ) -> BuckyResult<NDNQueryFileOutputResponse> {
        self.next.query_file(req).await.map_err(|e| {
            self.on_connect_failed(&e);
            e
        })
    }
}
