use crate::acl::*;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NONAclInputProcessor {
    acl: AclManagerRef,
    next: NONInputProcessorRef,
}

impl NONAclInputProcessor {
    pub fn new(acl: AclManagerRef, next: NONInputProcessorRef) -> NONInputProcessorRef {
        let ret = Self { acl, next };
        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONAclInputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::PutObject,

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
        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::GetObject,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Source(req.common.source.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: req.inner_path.clone(),

            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::PostObject,

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
        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        // TODO select暂时只允许同zone内使用
        self.acl
            .check_local_zone_permit("non in select_object", &req.common.source)
            .await?;

        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        let params = AclRequestParams {
            protocol: req.common.protocol.clone(),

            direction: AclDirection::In,
            operation: AclOperation::DeleteObject,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Source(req.common.source.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,
            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.delete_object(req).await
    }
}

pub(crate) struct NONAclOutputProcessor {
    protocol: NONProtocol,
    acl: AclManagerRef,
    target: DeviceId,
    next: NONOutputProcessorRef,
}

impl NONAclOutputProcessor {
    pub fn new(
        protocol: NONProtocol,
        acl: AclManagerRef,
        target: DeviceId,
        next: NONOutputProcessorRef,
    ) -> NONOutputProcessorRef {
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
impl NONOutputProcessor for NONAclOutputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectOutputRequest,
    ) -> BuckyResult<NONPutObjectOutputResponse> {
        // stack内部发起的output操作，一定存在object缓存字段
        let object = req.object.clone_object();

        let params = AclRequestParams {
            protocol: self.protocol.clone(),

            direction: AclDirection::Out,
            operation: AclOperation::PutObject,

            object_id: Some(req.object.object_id.clone()),
            object: Some(object),
            device_id: AclRequestDevice::Target(self.target.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,
            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectOutputRequest,
    ) -> BuckyResult<NONGetObjectOutputResponse> {
        let params = AclRequestParams {
            protocol: self.protocol.clone(),

            direction: AclDirection::Out,
            operation: AclOperation::GetObject,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Target(self.target.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: req.inner_path.clone(),
            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectOutputRequest,
    ) -> BuckyResult<NONPostObjectOutputResponse> {
        // stack内部发起的output操作，一定存在object缓存字段
        let object = req.object.clone_object();

        let params = AclRequestParams {
            protocol: self.protocol.clone(),

            direction: AclDirection::Out,
            operation: AclOperation::PostObject,

            object_id: Some(req.object.object_id.clone()),
            object: Some(object),
            device_id: AclRequestDevice::Target(self.target.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,
            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectOutputRequest,
    ) -> BuckyResult<NONSelectObjectOutputResponse> {
        // 对于output请求，select暂时不做限制
        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectOutputRequest,
    ) -> BuckyResult<NONDeleteObjectOutputResponse> {
        let params = AclRequestParams {
            protocol: self.protocol.clone(),

            direction: AclDirection::Out,
            operation: AclOperation::DeleteObject,

            object_id: Some(req.object_id.clone()),
            object: None,
            device_id: AclRequestDevice::Target(self.target.clone()),
            dec_id: req.common.dec_id.clone(),

            req_path: req.common.req_path.clone(),
            inner_path: None,
            referer_object: None,
        };

        let acl_req = self.acl.new_acl_request(params);

        self.acl.try_match_to_result(&acl_req).await?;
        self.next.delete_object(req).await
    }
}
