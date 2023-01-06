use crate::acl::AclManagerRef;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct GlobalStateAccessorAclInputProcessor {
    acl: AclManagerRef,
    next: GlobalStateAccessorInputProcessorRef,
}

impl GlobalStateAccessorAclInputProcessor {
    pub(crate) fn new(
        acl: AclManagerRef,
        next: GlobalStateAccessorInputProcessorRef,
    ) -> GlobalStateAccessorInputProcessorRef {
        let ret = Self { acl, next };

        Arc::new(Box::new(ret))
    }

    async fn check_access(
        &self,
        common: &RootStateInputRequestCommon,
        path: &str,
    ) -> BuckyResult<ObjectId> {
        let dec_id = match &common.target_dec_id {
            Some(dec_id) => dec_id,
            None => &common.source.dec,
        };

        if common.source.is_current_zone() {
            if common
                .source
                .check_target_dec_permission(&common.target_dec_id)
            {
                return Ok(dec_id.to_owned());
            }
        }

        let global_state = RequestGlobalStatePath {
            global_state_category: None,
            global_state_root: None,
            dec_id: Some(dec_id.to_owned()),
            req_path: Some(path.to_owned()),
        };

        self.acl
            .global_state_meta()
            .check_access(&common.source, &global_state, RequestOpType::Read)
            .await?;

        Ok(global_state.dec_id.unwrap())
    }
}

#[async_trait::async_trait]
impl GlobalStateAccessorInputProcessor for GlobalStateAccessorAclInputProcessor {
    async fn get_object_by_path(
        &self,
        mut req: RootStateAccessorGetObjectByPathInputRequest,
    ) -> BuckyResult<RootStateAccessorGetObjectByPathInputResponse> {
        // info!("get_object_by_path acl: {}", req);

        let dec_id = self.check_access(&req.common, &req.inner_path).await?;
        req.common.source.set_verified(dec_id);

        self.next.get_object_by_path(req).await
    }

    async fn list(
        &self,
        mut req: RootStateAccessorListInputRequest,
    ) -> BuckyResult<RootStateAccessorListInputResponse> {
        // info!("list acl: {}", req);

        let dec_id = self.check_access(&req.common, &req.inner_path).await?;
        req.common.source.set_verified(dec_id);

        self.next.list(req).await
    }
}
