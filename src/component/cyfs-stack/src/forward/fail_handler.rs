use crate::meta::ObjectFailHandler;
use cyfs_base::*;
use cyfs_lib::*;

use http_types::{Request, Response};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct HttpRequestorWithDeviceFailHandler {
    next: Arc<Box<dyn HttpRequestor>>,
    fail_handler: ObjectFailHandler,
    device_id: DeviceId,
}

impl HttpRequestorWithDeviceFailHandler {
    pub fn new(
        fail_handler: ObjectFailHandler,
        next: Box<dyn HttpRequestor>,
        device_id: DeviceId,
    ) -> Self {
        Self { next: Arc::new(next), fail_handler, device_id }
    }

    fn on_connect_failed(&self, e: &BuckyError) {
        if e.code() == BuckyErrorCode::ConnectFailed {
            self.fail_handler.on_device_fail(&self.device_id);
        }
    }
}

#[async_trait::async_trait]
impl HttpRequestor for HttpRequestorWithDeviceFailHandler {
    async fn request_ext(
        &self,
        req: &mut Option<Request>,
        conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response> {
        match self.next.request_ext(req, conn_info).await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                self.on_connect_failed(&e);
                Err(e)
            }
        }
    }

    fn remote_addr(&self) -> String {
        self.next.remote_addr()
    }

    fn remote_device(&self) -> Option<DeviceId> {
        self.next.remote_device()
    }

    fn clone_requestor(&self) -> Box<dyn HttpRequestor> {
        Box::new(self.clone())
    }

    async fn stop(&self) {
        self.next.stop().await
    }
}
