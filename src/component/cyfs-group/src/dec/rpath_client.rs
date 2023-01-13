use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, GroupMemberScope, NamedObject, ObjectDesc, ObjectId,
    RawConvertTo,
};
use cyfs_core::{
    GroupConsensusBlock, GroupProposal, GroupProposalObject, GroupRPath, GroupRPathStatus,
};
use cyfs_lib::NONObjectInfo;

use crate::CLIENT_POLL_TIMEOUT;

pub struct RPathClient {
    rpath: GroupRPath,
    local_id: ObjectId,
    non_driver: crate::network::NonDriver,
    network_sender: crate::network::Sender,
    network_listener: crate::network::Listener,
}

impl RPathClient {
    pub fn new(
        rpath: GroupRPath,
        local_id: ObjectId,
        non_driver: crate::network::NonDriver,
        network_sender: crate::network::Sender,
        network_listener: crate::network::Listener,
    ) -> Self {
        Self {
            rpath,
            non_driver,
            network_sender,
            network_listener,
            local_id,
        }
    }

    pub fn rpath(&self) -> &GroupRPath {
        &self.rpath
    }

    pub async fn post_proposal(
        &self,
        proposal: &GroupProposal,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        // TODO: signature
        let group = self
            .non_driver
            .get_group(proposal.r_path().group_id(), None)
            .await?;
        let admins = group.select_members_with_distance(&self.local_id, GroupMemberScope::Admin);
        let proposal_id = proposal.desc().object_id();
        let non_proposal = NONObjectInfo::new(proposal_id, proposal.to_vec()?, None);

        let waiter = self
            .network_listener
            .wait_proposal_result(proposal_id)
            .await?;
        let mut waiter_future = Some(waiter.wait());

        let mut post_result = None; // Err(BuckyError::new(BuckyErrorCode::Timeout, "timeout"));
        let mut exe_result = None;

        for admin in admins {
            match self
                .non_driver
                .post_object(non_proposal.clone(), admin)
                .await
            {
                Ok(r) => post_result = Some(Ok(())),
                Err(e) => {
                    if post_result.is_none() {
                        post_result = Some(Err(e));
                    }
                    continue;
                }
            }

            match futures::future::select(
                waiter_future.take().unwrap(),
                Box::pin(async_std::task::sleep(CLIENT_POLL_TIMEOUT)),
            )
            .await
            {
                futures::future::Either::Left((result, _)) => match result {
                    Err(_) => return Err(BuckyError::new(BuckyErrorCode::Unknown, "unknown")),
                    Ok(result) => match result {
                        Ok(result) => return Ok(result),
                        Err(e) => exe_result = Some(e),
                    },
                },
                futures::future::Either::Right((_, waiter)) => {
                    waiter_future = Some(waiter);
                }
            }
        }

        post_result.map_or(
            Err(BuckyError::new(BuckyErrorCode::InvalidTarget, "no admin")),
            |result| match result {
                Ok(_) => {
                    let err = exe_result
                        .map_or(BuckyError::new(BuckyErrorCode::Timeout, "timeout"), |e| e);
                    Err(err)
                }
                Err(e) => Err(e),
            },
        )
    }

    pub async fn get_field(&self, sub_path: &str) -> BuckyResult<GroupRPathStatus> {
        unimplemented!()
    }

    pub async fn get_block(&self, height: Option<u64>) -> BuckyResult<GroupConsensusBlock> {
        unimplemented!()
    }
}