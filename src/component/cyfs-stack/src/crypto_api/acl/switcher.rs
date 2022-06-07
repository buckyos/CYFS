use crate::crypto::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct CryptoInputAclSwitcher {
    acl_processor: CryptoInputProcessorRef,
    raw_processor: CryptoInputProcessorRef,
}

impl CryptoInputAclSwitcher {
    pub fn new(
        acl_processor: CryptoInputProcessorRef,
        raw_processor: CryptoInputProcessorRef,
    ) -> CryptoInputProcessorRef {
        let ret = Self {
            acl_processor,
            raw_processor,
        };

        Arc::new(Box::new(ret))
    }

    pub fn is_require_acl(&self, req_common: &CryptoInputRequestCommon) -> bool {
        req_common.protocol.is_require_acl()
    }

    pub fn get_processor(&self, req_common: &CryptoInputRequestCommon) -> &CryptoInputProcessorRef {
        match self.is_require_acl(req_common) {
            true => &self.acl_processor,
            false => &self.raw_processor,
        }
    }
}

#[async_trait::async_trait]
impl CryptoInputProcessor for CryptoInputAclSwitcher {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        self.get_processor(&req.common).verify_object(req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        self.get_processor(&req.common).sign_object(req).await
    }
}
