use crate::ndn::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NDNBdtEchoProcessor {
    response: BuckyError,
}

impl NDNBdtEchoProcessor {
    pub fn new() -> NDNInputProcessorRef {
        let ret = Self {
            response: BuckyError::from(BuckyErrorCode::NotImplement),
        };

        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNBdtEchoProcessor {
    async fn put_data(&self, _req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        Err(self.response.clone())
    }

    async fn get_data(&self, _req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        Err(self.response.clone())
    }

    async fn delete_data(
        &self,
        _req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        Err(self.response.clone())
    }

    async fn query_file(
        &self,
        _req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        Err(self.response.clone())
    }
}
