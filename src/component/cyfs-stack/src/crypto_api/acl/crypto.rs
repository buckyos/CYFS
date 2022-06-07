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
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::VerifyObject,

            object_id: Some(req.object.object_id.clone()),
            object: Some(req.object.clone_object()),
            device_id: AclRequestDevice::Source(req.common.source.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,
            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.verify_object(req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectInputRequest,
    ) -> BuckyResult<CryptoSignObjectInputResponse> {
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::SignObject,

            object_id: Some(req.object.object_id.clone()),
            object: Some(req.object.clone_object()),
            device_id: AclRequestDevice::Source(req.common.source.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,
            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.sign_object(req).await
    }
}

pub(crate) struct CryptoAclOutputProcessor {
    protocol: NONProtocol,
    acl: AclManagerRef,
    target: DeviceId,
    next: CryptoOutputProcessorRef,
}

impl CryptoAclOutputProcessor {
    pub fn new(
        protocol: NONProtocol,
        acl: AclManagerRef,
        target: DeviceId,
        next: CryptoOutputProcessorRef,
    ) -> CryptoOutputProcessorRef {
        let ret = Self {
            protocol,
            acl,
            target,
            next,
        };
        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl CryptoOutputProcessor for CryptoAclOutputProcessor {
    async fn verify_object(
        &self,
        req: CryptoVerifyObjectOutputRequest,
    ) -> BuckyResult<CryptoVerifyObjectOutputResponse> {
 
        let params = AclRequestParams {
            protocol: self.protocol.clone(),

            direction: AclDirection::Out,
            operation: AclOperation::VerifyObject,

            object_id: Some(req.object.object_id.clone()),
            object: Some(req.object.clone_object()),
            device_id: AclRequestDevice::Target(self.target.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,
            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;

        self.next.verify_object(req).await
    }

    async fn sign_object(
        &self,
        req: CryptoSignObjectOutputRequest,
    ) -> BuckyResult<CryptoSignObjectOutputResponse> {

        let params = AclRequestParams {
            protocol: self.protocol.clone(),

            direction: AclDirection::Out,
            operation: AclOperation::SignObject,

            object_id: Some(req.object.object_id.clone()),
            object: Some(req.object.clone_object()),
            device_id: AclRequestDevice::Target(self.target.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,
            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;

        self.next.sign_object(req).await
    }
}
