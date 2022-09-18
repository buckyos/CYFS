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

    async fn check_access(
        &self,
        name: &str,
        source: &RequestSourceInfo,
        op_type: RequestOpType,
    ) -> BuckyResult<()> {
        let path = format!("{}/{}/", CYFS_CRYPTO_VIRTUAL_PATH, name);
        let req_path = RequestGlobalStatePath::new_system_dec(Some(path));

        self.acl
            .global_state_meta()
            .check_access(source, &req_path, op_type)
            .await
    }
}

#[async_trait::async_trait]
impl CryptoInputProcessor for CryptoAclInputProcessor {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectInputRequest,
    ) -> BuckyResult<CryptoVerifyObjectInputResponse> {
        req.common.source.check_current_zone("crypto.verify_object")?;

        self.check_access("verify_object", &req.common.source, RequestOpType::Read).await?;

        self.next.verify_object(req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        req.common.source.check_current_zone("crypto.sign_object")?;

        self.check_access("sign_object", &req.common.source, RequestOpType::Write).await?;

        self.next.sign_object(req).await
    }
}