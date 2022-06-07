use crate::acl::*;
use crate::ndn::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NDNAclLocalInputProcessor {
    acl: AclManagerRef,
    next: NDNInputProcessorRef,
}

impl NDNAclLocalInputProcessor {
    pub fn new(acl: AclManagerRef, next: NDNInputProcessorRef) -> NDNInputProcessorRef {
        let ret = Self { acl, next };
        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNAclLocalInputProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        self.acl
            .check_local_permit("ndn in put_data", &req.common.source)
            .await?;

        self.next.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        self.acl
            .check_local_permit("ndn in get_data", &req.common.source)
            .await?;

        self.next.get_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        self.acl
            .check_local_permit("ndn in delete_data", &req.common.source)
            .await?;

        self.next.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        self.acl
            .check_local_permit("ndn in query_file", &req.common.source)
            .await?;

        self.next.query_file(req).await
    }
}
