use crate::acl::*;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct GlobalStateAccessAclInputProcessor {
    acl: AclManagerRef,
    next: GlobalStateAccessInputProcessorRef,
}

impl GlobalStateAccessAclInputProcessor {
    pub(crate) fn new(
        acl: AclManagerRef,
        next: GlobalStateAccessInputProcessorRef,
    ) -> GlobalStateAccessInputProcessorRef {
        let ret = Self { acl, next };

        Arc::new(Box::new(ret))
    }
}

#[async_trait::async_trait]
impl GlobalStateAccessInputProcessor for GlobalStateAccessAclInputProcessor {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathInputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathInputResponse> {
        // info!("get_object_by_path acl: {}", req);

        match req.common.protocol {
            NONProtocol::HttpLocalAuth | NONProtocol::HttpLocal => {
                self.next.get_object_by_path(req).await
            }
            _ => {
                let params = AclRequestParams {
                    protocol: req.common.protocol.clone(),

                    direction: AclDirection::In,
                    operation: AclOperation::ReadRootState,

                    object_id: None,
                    object: None,
                    device_id: AclRequestDevice::Source(req.common.source.clone()),
                    dec_id: req.common.dec_id.clone(),

                    req_path: Some(req.inner_path.clone()),
                    inner_path: None,
                    referer_object: None,
                };

                let acl_req = self.acl.new_acl_request(params);

                self.acl.try_match_to_result(&acl_req).await?;

                self.next.get_object_by_path(req).await
            }
        }
    }

    async fn list(
        &self,
        req: RootStateAccessListInputRequest,
    ) -> BuckyResult<RootStateAccessListInputResponse> {
        // info!("list acl: {}", req);

        match req.common.protocol {
            NONProtocol::HttpLocalAuth | NONProtocol::HttpLocal => self.next.list(req).await,
            _ => {
                let params = AclRequestParams {
                    protocol: req.common.protocol.clone(),

                    direction: AclDirection::In,
                    operation: AclOperation::ReadRootState,

                    object_id: None,
                    object: None,
                    device_id: AclRequestDevice::Source(req.common.source.clone()),
                    dec_id: req.common.dec_id.clone(),

                    req_path: Some(req.inner_path.clone()),
                    inner_path: None,
                    referer_object: None,
                };

                let acl_req = self.acl.new_acl_request(params);

                self.acl.try_match_to_result(&acl_req).await?;

                self.next.list(req).await
            }
        }
    }
}
