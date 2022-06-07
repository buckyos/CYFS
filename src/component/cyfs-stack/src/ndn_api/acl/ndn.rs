use crate::acl::*;
use crate::ndn::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NDNAclInputProcessor {
    acl: AclManagerRef,
    next: NDNInputProcessorRef,
}

impl NDNAclInputProcessor {
    pub fn new(acl: AclManagerRef, next: NDNInputProcessorRef) -> NDNInputProcessorRef {
        let ret = Self { acl, next };
        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNAclInputProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::PutData,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Source(req.common.source.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,

            referer_object: Some(req.common.referer_object.clone()),
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::GetData,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Source(req.common.source.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: req.inner_path.clone(),

            referer_object: Some(req.common.referer_object.clone()),
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.get_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::DeleteData,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Source(req.common.source.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,

            referer_object: Some(req.common.referer_object.clone()),
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::QueryFile,

            object_id: req.param.file_id(),
            object: None,
            device_id: AclRequestDevice::Source(req.common.source.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,

            referer_object: Some(req.common.referer_object.clone()),
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.query_file(req).await
    }
}

// ndn的内部使用input processor作为output
pub(crate) struct NDNAclOutputProcessor {
    acl: AclManagerRef,
    target: DeviceId,
    next: NDNInputProcessorRef,
}

impl NDNAclOutputProcessor {
    pub fn new(
        acl: AclManagerRef,
        target: DeviceId,
        next: NDNInputProcessorRef,
    ) -> NDNInputProcessorRef {
        let ret = Self { acl, target, next };
        Arc::new(Box::new(ret))
    }
}
#[async_trait::async_trait]
impl NDNInputProcessor for NDNAclOutputProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        let params = AclRequestParams {
            protocol: NONProtocol::DataBdt,

            direction: AclDirection::Out,
            operation: AclOperation::PutData,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Target(self.target.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,

            referer_object: Some(req.common.referer_object.clone()),
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        let params = AclRequestParams {
            protocol: NONProtocol::DataBdt,

            direction: AclDirection::Out,
            operation: AclOperation::GetData,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Source(self.target.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: req.inner_path.clone(),

            referer_object: Some(req.common.referer_object.clone()),
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.get_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        let params = AclRequestParams {
            protocol: NONProtocol::DataBdt,

            direction: AclDirection::Out,
            operation: AclOperation::DeleteData,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Source(self.target.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,

            referer_object: Some(req.common.referer_object.clone()),
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        let params = AclRequestParams {
            protocol: NONProtocol::DataBdt,

            direction: AclDirection::Out,
            operation: AclOperation::QueryFile,

            object_id: req.param.file_id(),
            object: None,
            device_id: AclRequestDevice::Source(req.common.source.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,

            referer_object: Some(req.common.referer_object.clone()),
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.query_file(req).await
    }
}
