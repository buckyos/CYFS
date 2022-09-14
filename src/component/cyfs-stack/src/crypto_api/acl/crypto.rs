use crate::acl::*;
use crate::crypto::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct CryptoAclInputProcessor {
    acl: AclManagerRef,
    next: CryptoInputProcessorRef,
}

impl CryptoAclInputProcessor {
    pub fn new(acl: AclManagerRef, next: CryptoInputProcessorRef) -> CryptoInputProcessorRef {
        let ret = Self { acl, next };
        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl CryptoInputProcessor for CryptoAclInputProcessor {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        req.common.source.check_current_zone("crypto.verify_object")?;

        self.next.verify_object(req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        // FIXME crypto使用system的.api/crypto/虚路径权限
        req.common.source.check_current_zone("crypto.sign_object")?;

        self.next.sign_object(req).await
    }
}