use crate::ndn::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NDNInputAclSwitcher {
    acl_processor: NDNInputProcessorRef,
    raw_processor: NDNInputProcessorRef,
}

impl NDNInputAclSwitcher {
    pub fn new(
        acl_processor: NDNInputProcessorRef,
        raw_processor: NDNInputProcessorRef,
    ) -> NDNInputProcessorRef {
        let ret = Self {
            acl_processor,
            raw_processor,
        };

        Arc::new(Box::new(ret))
    }

    pub fn is_require_acl(&self, req_common: &NDNInputRequestCommon) -> bool {
        req_common.protocol.is_require_acl()
    }

    pub fn get_processor(&self, req_common: &NDNInputRequestCommon) -> &NDNInputProcessorRef {
        match self.is_require_acl(req_common) {
            true => &self.acl_processor,
            false => &self.raw_processor,
        }
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNInputAclSwitcher {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        self.get_processor(&req.common).put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        self.get_processor(&req.common).get_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        self.get_processor(&req.common).delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        self.get_processor(&req.common).query_file(req).await
    }
}
