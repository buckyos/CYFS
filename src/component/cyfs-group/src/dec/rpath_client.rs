use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, GroupMemberScope, NamedObject, ObjectDesc, ObjectId,
    RawConvertTo,
};
use cyfs_core::{
    GroupConsensusBlock, GroupProposal, GroupProposalObject, GroupRPath, GroupRPathStatus,
};
use cyfs_lib::NONObjectInfo;
use rand::Rng;

use crate::{
    dec_state::DecStateSynchronizer, storage::DecStorage, HotstuffMessage, CLIENT_POLL_TIMEOUT,
};

pub struct RPathClient {
    rpath: GroupRPath,
    local_id: ObjectId,
    non_driver: crate::network::NonDriver,
    network_sender: crate::network::Sender,
    network_listener: crate::network::Listener,
    state_sync: DecStateSynchronizer,
}

impl RPathClient {
    pub fn new(
        rpath: GroupRPath,
        local_id: ObjectId,
        non_driver: crate::network::NonDriver,
        network_sender: crate::network::Sender,
        network_listener: crate::network::Listener,
        dec_store: DecStorage,
    ) -> Self {
        let state_sync = DecStateSynchronizer::new(
            local_id,
            rpath.clone(),
            non_driver.clone(),
            dec_store.clone(),
        );

        Self {
            rpath,
            non_driver,
            network_sender,
            network_listener,
            local_id,
            state_sync,
        }
    }

    pub fn rpath(&self) -> &GroupRPath {
        &self.rpath
    }

    pub async fn post_proposal(
        &self,
        proposal: &GroupProposal,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        assert_eq!(proposal.r_path(), &self.rpath);

        // TODO: signature
        let group = self
            .non_driver
            .get_group(proposal.r_path().group_id(), None)
            .await?;
        let admins = group.select_members_with_distance(&self.local_id, GroupMemberScope::Admin);
        let proposal_id = proposal.desc().object_id();
        let non_proposal = NONObjectInfo::new(proposal_id, proposal.to_vec()?, None);

        let waiter = self.state_sync.wait_proposal_result(proposal_id).await;
        let mut waiter_future = Some(waiter.wait());

        let mut post_result = None;
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

    // request last state from random admin
    pub async fn refresh_state(&self) -> BuckyResult<()> {
        let group = self
            .non_driver
            .get_group(&self.rpath.group_id(), None)
            .await?;

        let admins = group.select_members_with_distance(&self.local_id, GroupMemberScope::Admin);
        let random = rand::thread_rng().gen_range(0..admins.len());
        let admin = admins.get(random).unwrap().clone();

        self.network_sender
            .post_message(HotstuffMessage::LastStateRequest, self.rpath.clone(), admin)
            .await;
        Ok(())
    }

    pub async fn get_by_path(&self, sub_path: &str) -> BuckyResult<ObjectId> {
        let group = self
            .non_driver
            .get_group(self.rpath().group_id(), None)
            .await?;

        let members = group.select_members_with_distance(&self.local_id, GroupMemberScope::All);
        let req_msg = HotstuffMessage::QueryState(sub_path.to_string());

        let waiter = self
            .network_listener
            .wait_query_state(sub_path.to_string(), self.rpath.clone())
            .await?;
        let mut waiter_future = Some(waiter.wait());

        let mut exe_result = None;

        for member in members {
            self.network_sender
                .post_message(req_msg.clone(), self.rpath.clone(), member)
                .await;

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

        let err = exe_result.map_or(BuckyError::new(BuckyErrorCode::Timeout, "timeout"), |e| e);
        Err(err)
    }

    pub async fn get_block(&self, height: Option<u64>) -> BuckyResult<GroupConsensusBlock> {
        unimplemented!()
    }
}
