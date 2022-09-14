use crate::ndn::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NDNAclLocalInputProcessor {
    next: NDNInputProcessorRef,
}

impl NDNAclLocalInputProcessor {
    pub fn new(next: NDNInputProcessorRef) -> NDNInputProcessorRef {
        let ret = Self { next };
        Arc::new(Box::new(ret))
    }

    fn check_access(&self, service: &str, common: &NDNInputRequestCommon) -> BuckyResult<()> {
        common.source.check_current_device(service)
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNAclLocalInputProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        self.check_access("ndn.put_data", &req.common)?;

        self.next.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        self.check_access("ndn.get_data", &req.common)?;

        self.next.get_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        self.check_access("ndn.delete_data", &req.common)?;

        self.next.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        self.check_access("ndn.query_file", &req.common)?;

        self.next.query_file(req).await
    }
}
