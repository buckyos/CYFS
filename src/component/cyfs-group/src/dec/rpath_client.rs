use std::sync::Arc;

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, GroupMemberScope, NamedObject, ObjectDesc, ObjectId,
    RawConvertTo,
};
use cyfs_core::{GroupConsensusBlock, GroupProposal, GroupProposalObject, GroupRPath};
use cyfs_lib::{GlobalStateRawProcessorRef, NONObjectInfo};
use rand::Rng;

use crate::{
    dec_state::{DecStateRequestor, DecStateSynchronizer},
    storage::{DecStorage, GroupShellManager},
    Committee, HotstuffMessage, CLIENT_POLL_TIMEOUT,
};

struct RPathClientRaw {
    rpath: GroupRPath,
    local_device_id: ObjectId,
    non_driver: crate::network::NONDriverHelper,
    shell_mgr: GroupShellManager,
    network_sender: crate::network::Sender,
    state_sync: DecStateSynchronizer,
    state_requestor: DecStateRequestor,
}

#[derive(Clone)]
pub struct RPathClient(Arc<RPathClientRaw>);

impl RPathClient {
    pub(crate) async fn load(
        local_device_id: ObjectId,
        rpath: GroupRPath,
        state_processor: GlobalStateRawProcessorRef,
        non_driver: crate::network::NONDriverHelper,
        shell_mgr: GroupShellManager,
        network_sender: crate::network::Sender,
    ) -> BuckyResult<Self> {
        let dec_store = DecStorage::load(state_processor).await?;
        let committee = Committee::new(
            rpath.group_id().clone(),
            non_driver.clone(),
            shell_mgr.clone(),
            local_device_id,
        );

        let state_sync = DecStateSynchronizer::new(
            local_device_id,
            rpath.clone(),
            committee.clone(),
            non_driver.clone(),
            shell_mgr.clone(),
            dec_store.clone(),
        );

        let state_requestor = DecStateRequestor::new(
            local_device_id,
            rpath.clone(),
            committee,
            network_sender.clone(),
            non_driver.clone(),
            dec_store.clone(),
        );

        let raw = RPathClientRaw {
            rpath,
            non_driver,
            network_sender,
            local_device_id,
            state_sync,
            state_requestor,
            shell_mgr,
        };

        Ok(Self(Arc::new(raw)))
    }

    pub fn rpath(&self) -> &GroupRPath {
        &self.0.rpath
    }

    pub async fn post_proposal(
        &self,
        proposal: &GroupProposal,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        assert_eq!(proposal.rpath(), &self.0.rpath);

        // TODO: signature
        let group = self
            .0
            .shell_mgr
            .get_group(proposal.rpath().group_id(), None, None)
            .await?;
        let oods = group.ood_list_with_distance(&self.0.local_device_id);
        let proposal_id = proposal.desc().object_id();
        let non_proposal = NONObjectInfo::new(proposal_id, proposal.to_vec()?, None);

        let waiter = self.0.state_sync.wait_proposal_result(proposal_id).await;
        let mut waiter_future = Some(waiter.wait());

        let mut post_result = None;
        let mut exe_result = None;

        for ood in oods {
            match self
                .0
                .non_driver
                .post_object(non_proposal.clone(), Some(ood))
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

    // request last state from random ood in group.ood_list()
    pub async fn refresh_state(&self) -> BuckyResult<()> {
        let group = self
            .0
            .shell_mgr
            .get_group(&self.0.rpath.group_id(), None, None)
            .await?;

        let oods = group.ood_list_with_distance(&self.0.local_device_id);
        let random = rand::thread_rng().gen_range(0..oods.len());
        let ood = oods.get(random).unwrap().clone();

        self.0
            .network_sender
            .post_message(HotstuffMessage::LastStateRequest, self.0.rpath.clone(), ood)
            .await;
        Ok(())
    }

    pub async fn get_by_path(&self, sub_path: &str) -> BuckyResult<Option<NONObjectInfo>> {
        let group = self
            .0
            .shell_mgr
            .get_group(self.0.rpath.group_id(), None, None)
            .await?;

        let members =
            group.select_members_with_distance(&self.0.local_device_id, GroupMemberScope::All);
        let req_msg = HotstuffMessage::QueryState(sub_path.to_string());

        let waiter = self
            .0
            .state_requestor
            .wait_query_state(sub_path.to_string())
            .await;
        let mut waiter_future = Some(waiter.wait());

        let mut exe_result = None;

        for member in members {
            self.0
                .network_sender
                .post_message(req_msg.clone(), self.0.rpath.clone(), member)
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

    pub(crate) async fn on_message(&self, msg: HotstuffMessage, remote: ObjectId) {
        match msg {
            HotstuffMessage::Block(block) => unreachable!(),
            HotstuffMessage::BlockVote(vote) => unreachable!(),
            HotstuffMessage::TimeoutVote(vote) => unreachable!(),
            HotstuffMessage::Timeout(tc) => unreachable!(),
            HotstuffMessage::SyncRequest(min_bound, max_bound) => unreachable!(),
            HotstuffMessage::LastStateRequest => unreachable!(),
            HotstuffMessage::StateChangeNotify(header_block, qc) => {
                self.0
                    .state_sync
                    .on_state_change(header_block, qc, remote)
                    .await
            }
            HotstuffMessage::ProposalResult(proposal_id, result) => {
                self.0
                    .state_sync
                    .on_proposal_complete(proposal_id, result, remote)
                    .await
            }
            HotstuffMessage::QueryState(sub_path) => {
                self.0
                    .state_requestor
                    .on_query_state(sub_path, remote)
                    .await
            }
            HotstuffMessage::VerifiableState(sub_path, result) => {
                self.0
                    .state_requestor
                    .on_verifiable_state(sub_path, result, remote)
                    .await
            }
        }
    }
}
