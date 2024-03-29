use std::sync::Arc;

use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, NamedObject, ObjectDesc};
use cyfs_core::{GroupProposalObject};
use cyfs_group::GroupManager;
use cyfs_group_lib::{
    GroupPushProposalInputRequest, GroupPushProposalInputResponse,
    GroupStartServiceInputRequest, GroupStartServiceInputResponse,
};

use crate::group::{GroupInputProcessor, GroupInputProcessorRef};

#[derive(Clone)]
pub(crate) struct LocalGroupService {
    group_manager: GroupManager,
}

impl LocalGroupService {
    pub(crate) fn new(group_manager: GroupManager) -> Self {
        Self { group_manager }
    }

    pub fn clone_processor(&self) -> GroupInputProcessorRef {
        Arc::new(self.clone())
    }
}

#[async_trait::async_trait]
impl GroupInputProcessor for LocalGroupService {
    async fn start_service(
        &self,
        req: GroupStartServiceInputRequest,
    ) -> BuckyResult<GroupStartServiceInputResponse> {
        self.group_manager
            .find_rpath_service(
                &req.group_id,
                &req.common.source.dec,
                req.rpath.as_str(),
                true,
            )
            .await
            .map(|_| GroupStartServiceInputResponse {})
            .map_err(|err| {
                log::error!(
                    "group start service {}-{}-{} failed {:?}",
                    req.group_id,
                    req.common.source.dec,
                    req.rpath,
                    err
                );
                err
            })
    }

    async fn push_proposal(
        &self,
        req: GroupPushProposalInputRequest,
    ) -> BuckyResult<GroupPushProposalInputResponse> {
        let proposal_id = req.proposal.desc().object_id();
        let rpath = req.proposal.rpath().clone();
        if &req.common.source.dec != rpath.dec_id() {
            let msg = format!(
                "group push proposal {}-{}-{} {} failed: the source dec({}) should be same as that in GroupProposal object",
                rpath.group_id(),
                rpath.dec_id(),
                rpath.rpath(),
                proposal_id,
                req.common.source.dec
            );
            log::error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        let service = self
            .group_manager
            .find_rpath_service(
                rpath.group_id(),
                &req.common.source.dec,
                rpath.rpath(),
                true,
            )
            .await
            .map_err(|err| {
                log::error!(
                    "group push proposal {}-{}-{} {} failed when find the service {:?}",
                    rpath.group_id(),
                    rpath.dec_id(),
                    rpath.rpath(),
                    proposal_id,
                    err
                );
                err
            })?;

        service
            .push_proposal(req.proposal)
            .await
            .map(|object| GroupPushProposalInputResponse { object })
            .map_err(|err| {
                log::error!(
                    "group push proposal {}-{}-{} {} failed {:?}",
                    rpath.group_id(),
                    rpath.dec_id(),
                    rpath.rpath(),
                    proposal_id,
                    err
                );
                err
            })
    }
}
