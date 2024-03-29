use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};

use async_std::channel::{Receiver, Sender};
use cyfs_base::{
    bucky_time_to_system_time, BuckyError, BuckyErrorCode, BuckyResult, Group, NamedObject,
    ObjectDesc, ObjectId, ObjectLink, ObjectTypeCode, OwnerObjectDesc, RawConvertTo, RawDecode,
    RawEncode, RsaCPUObjectSigner, SignatureSource, Signer,
};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, GroupConsensusBlockProposal, GroupProposal,
    GroupProposalObject, GroupRPath, HotstuffBlockQC, HotstuffTimeout, ToGroupShell,
};
use cyfs_group_lib::{ExecuteResult, HotstuffBlockQCVote, HotstuffTimeoutVote};
use cyfs_lib::NONObjectInfo;
use futures::FutureExt;
use itertools::Itertools;

use crate::{
    consensus::synchronizer::Synchronizer,
    dec_state::{CallReplyNotifier, CallReplyWaiter, StatePusher},
    helper::Timer,
    storage::GroupShellManager,
    Committee, GroupObjectMapProcessor, GroupStorage, HotstuffMessage, PendingProposalConsumer,
    RPathEventNotifier, SyncBound, VoteMgr, VoteThresholded, BLOCK_COUNT_REST_TO_SYNC,
    CHANNEL_CAPACITY, GROUP_DEFAULT_CONSENSUS_INTERVAL, HOTSTUFF_TIMEOUT_DEFAULT,
    PROPOSAL_MAX_TIMEOUT, TIME_PRECISION,
};

/**
 * TODO: generate empty block when the 'Node' is synchronizing
 *
 * How to distinguish synchronizing: max_quorum_round - round > BLOCK_COUNT_REST_TO_SYNC
*/

pub(crate) struct Hotstuff {
    rpath: GroupRPath,
    local_device_id: ObjectId,
    tx_message: Sender<(HotstuffMessage, ObjectId)>,
    state_pusher: StatePusher,
    proposal_result_notifier: CallReplyNotifier<ObjectId, BuckyResult<Option<NONObjectInfo>>>,
}

impl std::fmt::Debug for Hotstuff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}-{:?}", self.rpath, self.local_device_id)
    }
}

impl Hotstuff {
    pub fn new(
        local_id: ObjectId,
        local_device_id: ObjectId,
        committee: Committee,
        store: GroupStorage,
        signer: Arc<RsaCPUObjectSigner>,
        network_sender: crate::network::Sender,
        non_driver: crate::network::NONDriverHelper,
        shell_mgr: GroupShellManager,
        proposal_consumer: PendingProposalConsumer,
        event_notifier: RPathEventNotifier,
        rpath: GroupRPath,
    ) -> Self {
        let (tx_message, rx_message) = async_std::channel::bounded(CHANNEL_CAPACITY);
        let proposal_result_notifier = CallReplyNotifier::new();

        let state_pusher = StatePusher::new(
            local_id,
            network_sender.clone(),
            rpath.clone(),
            non_driver.clone(),
            shell_mgr.clone(),
        );

        let mut runner = HotstuffRunner::new(
            local_id,
            local_device_id,
            committee,
            store,
            signer,
            network_sender,
            non_driver,
            shell_mgr,
            tx_message.clone(),
            rx_message,
            proposal_consumer,
            state_pusher.clone(),
            event_notifier,
            rpath.clone(),
            proposal_result_notifier.clone(),
        );

        async_std::task::spawn(async move { runner.run().await });

        Self {
            local_device_id,
            tx_message,
            state_pusher,
            rpath,
            proposal_result_notifier,
        }
    }

    pub async fn wait_proposal_result(
        &self,
        proposal_id: ObjectId,
    ) -> CallReplyWaiter<BuckyResult<Option<NONObjectInfo>>> {
        self.proposal_result_notifier.prepare(proposal_id).await
    }

    pub async fn on_block(&self, block: cyfs_core::GroupConsensusBlock, remote: ObjectId) {
        let msg = format!("[hotstuff] local: {:?}, on_block: {:?}/{:?}/{:?}, prev: {:?}/{:?}, owner: {:?}, remote: {:?}.",
            self,
            block.block_id(), block.round(), block.height(),
            block.prev_block_id(), block.qc().as_ref().map_or(0, |qc| qc.round),
            block.owner(), remote);

        log::debug!("{}", msg);

        if let Err(err) = self
            .tx_message
            .send((HotstuffMessage::Block(block.clone()), remote))
            .await
        {
            log::warn!("{} send message failed, may lost, err={:?}.", msg, err);
        }
    }

    pub async fn on_block_vote(&self, vote: HotstuffBlockQCVote, remote: ObjectId) {
        let msg = format!("[hotstuff] local: {:?}, on_block_vote: {:?}/{:?}, prev: {:?}, voter: {:?}, remote: {:?}.",
            self,
            vote.block_id, vote.round,
            vote.prev_block_id,
            vote.voter, remote);

        log::debug!("{}", msg);

        if let Err(err) = self
            .tx_message
            .send((HotstuffMessage::BlockVote(vote.clone()), remote))
            .await
        {
            log::warn!("{} send message failed, may lost, err={:?}.", msg, err);
        }
    }

    pub async fn on_timeout_vote(&self, vote: HotstuffTimeoutVote, remote: ObjectId) {
        let msg = format!(
            "[hotstuff] local: {:?}, on_timeout_vote: {:?}, qc: {:?}, voter: {:?}, remote: {:?}.",
            self,
            vote.round,
            vote.high_qc.as_ref().map(|qc| format!(
                "{:?}/{:?}/{:?}/{:?}",
                qc.block_id,
                qc.round,
                qc.prev_block_id,
                qc.votes
                    .iter()
                    .map(|v| v.voter.to_string())
                    .collect::<Vec<String>>()
            )),
            vote.voter,
            remote
        );

        log::debug!("{}", msg);

        if let Err(err) = self
            .tx_message
            .send((HotstuffMessage::TimeoutVote(vote.clone()), remote))
            .await
        {
            log::warn!("{} send message failed, may lost, err={:?}.", msg, err);
        }
    }

    pub async fn on_timeout(&self, tc: HotstuffTimeout, remote: ObjectId) {
        let msg = format!(
            "[hotstuff] local: {:?}, on_timeout: {:?}, voter: {:?}, remote: {:?}.",
            self,
            tc.round,
            tc.votes
                .iter()
                .map(|vote| format!("{:?}/{:?}", vote.high_qc_round, vote.voter,))
                .collect::<Vec<String>>(),
            remote
        );

        log::debug!("{}", msg);

        if let Err(err) = self
            .tx_message
            .send((HotstuffMessage::Timeout(tc), remote))
            .await
        {
            log::warn!("{} send message failed, may lost, err={:?}.", msg, err);
        }
    }

    pub async fn on_sync_request(
        &self,
        min_bound: SyncBound,
        max_bound: SyncBound,
        remote: ObjectId,
    ) {
        let msg = format!(
            "[hotstuff] local: {:?}, on_sync_request: min: {:?}, max: {:?}, remote: {:?}.",
            self, min_bound, max_bound, remote
        );

        log::debug!("{}", msg);

        if let Err(err) = self
            .tx_message
            .send((HotstuffMessage::SyncRequest(min_bound, max_bound), remote))
            .await
        {
            log::warn!("{} send message failed, may lost, err={:?}.", msg, err);
        }
    }

    pub async fn request_last_state(&self, remote: ObjectId) {
        log::debug!(
            "[hotstuff] local: {:?}, on_sync_request: remote: {:?},",
            self,
            remote
        );

        self.state_pusher.request_last_state(remote).await;
    }

    pub async fn on_query_state(&self, sub_path: String, remote: ObjectId) {
        let msg = format!(
            "[hotstuff] local: {:?}, on_query_state: sub_path: {}, remote: {:?}.",
            self, sub_path, remote
        );

        log::debug!("{}", msg);

        if let Err(err) = self
            .tx_message
            .send((HotstuffMessage::QueryState(sub_path), remote))
            .await
        {
            log::warn!("{} send message failed, may lost, err={:?}.", msg, err);
        }
    }
}

struct HotstuffRunner {
    local_id: ObjectId,
    local_device_id: ObjectId,
    committee: Committee,
    store: GroupStorage,
    signer: Arc<RsaCPUObjectSigner>,
    round: u64,                       // 当前轮次
    high_qc: Option<HotstuffBlockQC>, // 最后一次通过投票的确认信息
    tc: Option<HotstuffTimeout>,
    max_quorum_round: u64,
    max_quorum_height: u64,
    timer: Timer, // 定时器
    vote_mgr: VoteMgr,
    network_sender: crate::network::Sender,
    non_driver: crate::network::NONDriverHelper,
    shell_mgr: GroupShellManager,
    tx_message: Sender<(HotstuffMessage, ObjectId)>,
    rx_message: Receiver<(HotstuffMessage, ObjectId)>,
    tx_message_inner: Sender<(GroupConsensusBlock, ObjectId)>,
    rx_message_inner: Receiver<(GroupConsensusBlock, ObjectId)>,
    tx_block_gen: Sender<(GroupConsensusBlock, HashMap<ObjectId, GroupProposal>)>,
    rx_block_gen: Receiver<(GroupConsensusBlock, HashMap<ObjectId, GroupProposal>)>,
    proposal_consumer: PendingProposalConsumer,
    event_notifier: RPathEventNotifier,
    synchronizer: Synchronizer,
    rpath: GroupRPath,
    rx_proposal_waiter: Option<(Receiver<()>, u64)>,
    state_pusher: StatePusher,
    proposal_result_notifier: CallReplyNotifier<ObjectId, BuckyResult<Option<NONObjectInfo>>>,
}

impl std::fmt::Debug for HotstuffRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.debug_identify())
    }
}

impl HotstuffRunner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_id: ObjectId,
        local_device_id: ObjectId,
        committee: Committee,
        store: GroupStorage,
        signer: Arc<RsaCPUObjectSigner>,
        network_sender: crate::network::Sender,
        non_driver: crate::network::NONDriverHelper,
        shell_mgr: GroupShellManager,
        tx_message: Sender<(HotstuffMessage, ObjectId)>,
        rx_message: Receiver<(HotstuffMessage, ObjectId)>,
        proposal_consumer: PendingProposalConsumer,
        state_pusher: StatePusher,
        event_notifier: RPathEventNotifier,
        rpath: GroupRPath,
        proposal_result_notifier: CallReplyNotifier<ObjectId, BuckyResult<Option<NONObjectInfo>>>,
    ) -> Self {
        let max_round_block = store.block_with_max_round();
        let last_qc = store.last_qc();
        let last_tc = store.last_tc();

        let last_vote_round = store.last_vote_round();
        let block_quorum_round = last_qc.as_ref().map_or(0, |qc| qc.round);
        let timeout_quorum_round = last_tc.as_ref().map_or(0, |tc| tc.round);
        let quorum_round = block_quorum_round.max(timeout_quorum_round);
        let (max_round_block_round, max_round_qc_round) =
            max_round_block.as_ref().map_or((0, 0), |block| {
                let qc_round = block.qc().as_ref().map_or(0, |qc| qc.round);
                (block.round(), qc_round)
            });
        let round = last_vote_round
            .max(quorum_round + 1)
            .max(max_round_block_round);

        log::debug!("[hotstuff] local: {:?}-{:?}-{}, startup with last_vote_round = {}, quorum_round = {}/{}, max_round_block_round = {}/{}"
            , rpath, local_device_id, round, last_vote_round, block_quorum_round, timeout_quorum_round, max_round_block_round, max_round_qc_round);

        let high_qc = if max_round_qc_round >= block_quorum_round {
            max_round_block.map_or(None, |b| b.qc().clone())
        } else {
            last_qc.clone()
        };

        let tc = last_tc.clone();

        let vote_mgr = VoteMgr::new(committee.clone(), round);
        let init_timer_interval = GROUP_DEFAULT_CONSENSUS_INTERVAL;
        let max_quorum_round = round - 1;
        let header_height = store.header_height();
        let max_quorum_height = if header_height == 0 {
            0
        } else {
            header_height + 1
        };

        let (tx_block_gen, rx_block_gen) = async_std::channel::bounded(1);
        let (tx_message_inner, rx_message_inner) = async_std::channel::bounded(CHANNEL_CAPACITY);

        let synchronizer = Synchronizer::new(
            local_device_id,
            network_sender.clone(),
            rpath.clone(),
            store.max_height(),
            store.max_round(),
            tx_message_inner.clone(),
        );

        Self {
            local_id,
            local_device_id,
            committee,
            store,
            signer,
            round,
            high_qc,
            timer: Timer::new(init_timer_interval),
            vote_mgr,
            network_sender,
            tx_message,
            rx_message,
            event_notifier,
            synchronizer,
            non_driver,
            rpath,
            proposal_consumer,
            rx_proposal_waiter: None,
            tc,
            max_quorum_round,
            max_quorum_height,
            state_pusher,
            tx_block_gen,
            rx_block_gen,
            proposal_result_notifier,
            tx_message_inner,
            rx_message_inner,
            shell_mgr,
        }
    }

    async fn handle_block(
        &mut self,
        block: &GroupConsensusBlock,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        log::debug!("[hotstuff] local: {:?}, handle_block: {:?}/{:?}/{:?}, prev: {:?}/{:?}, owner: {:?}, remote: {:?},",
            self,
            block.block_id(), block.round(), block.height(),
            block.prev_block_id(), block.qc().as_ref().map_or(0, |qc| qc.round),
            block.owner(), remote);

        if block.height() <= self.store.header_height() {
            log::warn!(
                "[hotstuff] local: {:?}, handle_block: {:?}/{:?}/{:?} ignored for expired.",
                self,
                block.block_id(),
                block.round(),
                block.height(),
            );
            return Err(BuckyError::new(BuckyErrorCode::Expired, "block expired"));
        }

        let latest_group = self
            .shell_mgr
            .get_group(self.rpath.group_id(), None, Some(&remote))
            .await?;
        if !latest_group.contain_ood(&remote) {
            log::warn!(
                "[hotstuff] local: {:?}, receive block({}) from unknown({})",
                self,
                block.block_id(),
                remote
            );
            return Ok(());
        }

        // 1. verify the signatures of the block
        // 2. verify the block generator
        // 3. synchronize the block
        // 4. verify the result of every proposal

        Self::check_block_result_state(block)?;

        log::debug!(
            "[hotstuff] local: {:?}, handle_block-step2: {:?}",
            self,
            block.block_id()
        );

        {
            // check leader
            let leader_owner = self
                .get_leader_owner(Some(block.group_shell_id()), block.round(), Some(&remote))
                .await?;

            if &leader_owner != block.owner() {
                log::warn!("[hotstuff] local: {:?}, receive block({:?}) from invalid leader({}), expected {:?}",
                    self,
                    block.block_id(),
                    block.owner(),
                    leader_owner
                );
                return Err(BuckyError::new(BuckyErrorCode::Ignored, "invalid leader"));
            }
        }

        self.committee
            .verify_block(block, remote)
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, verify block {:?} failed, {:?}.",
                    self,
                    block.block_id(),
                    err
                );
                err
            })?;

        let quorum_round = block.qc().as_ref().map_or(0, |qc| qc.round);
        self.update_max_quorum_round(quorum_round);
        self.update_max_quorum_height(block.height() - 1);

        log::debug!(
            "[hotstuff] local: {:?}, handle_block-step3: {:?}",
            self,
            block.block_id()
        );

        let prev_block = match self.check_block_linked(&block, remote).await {
            Ok(link) => link,
            Err(err) => return err,
        };

        log::debug!(
            "[hotstuff] local: {:?}, handle_block-step4: {:?}",
            self,
            block.block_id()
        );

        {
            // 1. verify the results for proposals by DecApp
            // 2. rebuild the result-state in on_verify by DecApp
            // TODO:
            // We can accept the block only for signatures enough without verify from `DecApp`.
            // The `QC` is valuable for confirm the previous block.
            // But we should not make a vote to the block not verified by `DecApp`.
            // So we should only verify the block from `DecApp` before it will be voted, not here.
            // But I cannot download the result-state with downloading from remote,
            // so I need to verify it here to generate the result-state one by one in the `DecApp.on_verify`.
            // If we can download the result-state tree to rebuild it, we can remove the code in this scope.
            let mut proposals = HashMap::default();
            self
                .non_driver
                .load_all_proposals_for_block(block, &mut proposals)
                .await.map_err(|err| {
                    log::warn!("[hotstuff] local: {:?}, make vote to block {} ignore for load proposals failed {:?}",
                        self,
                        block.block_id(),
                        err
                    );
                    err
                })?;

            self.check_block_proposal_result_state_by_app(block, &proposals, &prev_block, &remote)
                .await
                .map_err(|err| {
                    log::warn!(
                        "[hotstuff] local: {:?}, make vote to block {} ignore for app verify failed {:?}",
                        self,
                        block.block_id(),
                        err);
                    err
                })?;
        }

        self.synchronizer.pop_link_from(block);

        self.process_qc(block.qc()).await;

        if let Some(tc) = block.tc() {
            self.advance_round(tc.round).await;
        }

        self.process_block(block, remote, &HashMap::new()).await
    }

    fn check_block_result_state(block: &GroupConsensusBlock) -> BuckyResult<()> {
        if let Some(last_proposal) = block.proposals().last() {
            if &last_proposal.result_state != block.result_state_id() {
                log::warn!("the result-state({:?}) in last-proposal is unmatch with block.result_state_id({:?})"
                    , last_proposal.result_state, block.result_state_id());
                return Err(BuckyError::new(
                    BuckyErrorCode::Unmatch,
                    "result-state unmatch",
                ));
            }
        }
        Ok(())
    }

    fn check_empty_block_result_state_with_prev(
        block: &GroupConsensusBlock,
        prev_block: &Option<GroupConsensusBlock>,
    ) -> BuckyResult<()> {
        if block.proposals().is_empty() {
            match prev_block.as_ref() {
                Some(prev_block) => {
                    if block.result_state_id() != prev_block.result_state_id() {
                        log::warn!("block.result_state_id({:?}) is unmatch with prev_block.result_state_id({:?}) with no proposal."
                            , block.result_state_id(), prev_block.result_state_id());

                        return Err(BuckyError::new(
                            BuckyErrorCode::Unmatch,
                            "result-state unmatch",
                        ));
                    }
                }
                None => {
                    log::warn!("the first block is empty, ignore.");
                    return Err(BuckyError::new(
                        BuckyErrorCode::Ignored,
                        "empty first block",
                    ));
                }
            }
        }

        Ok(())
    }

    #[async_recursion::async_recursion]
    async fn check_block_proposal_result_state_by_app(
        &self,
        block: &GroupConsensusBlock,
        proposals: &HashMap<ObjectId, GroupProposal>,
        prev_block: &Option<GroupConsensusBlock>,
        remote: &ObjectId,
    ) -> BuckyResult<()> {
        let mut prev_state_id = match prev_block.as_ref() {
            Some(prev_block) => {
                let result_state_id = prev_block.result_state_id();
                if let Some(result_state_id) = result_state_id {
                    if let Err(err) = self
                        .make_sure_result_state(result_state_id, &[prev_block.owner(), remote])
                        .await
                    {
                        // TODO
                        // try rebuild the result-state for prev-block, it'll unreachable usually except something broken.
                        // if we can successfully rebuild the result-state in the future, we can remove the code in this part.
                        match prev_block.prev_block_id() {
                            Some(prev_prev_block_id) => {
                                let prev_prev_block =
                                    self.non_driver.get_block(prev_prev_block_id, None).await.map_err(|err| {
                                        log::warn!("[hotstuff] local: {:?}, rebuild of prev-prev-block({:?}) failed, get prev-prev-block failed {:?}."
                                            , self, prev_prev_block_id, err);
                                        err
                                    })?;

                                let mut prev_proposals = HashMap::new();
                                for proposal_ex_info in prev_block.proposals() {
                                    // TODO: Need a method to allow access in `Group`, Now only get from the owner(AccessString::full())
                                    let proposal = self
                                        .non_driver
                                        .get_proposal(&proposal_ex_info.proposal, Some(prev_block.owner()))
                                        .await.map_err(|err| {
                                            log::warn!("[hotstuff] local: {:?}, rebuild of prev-prev-block({:?}) failed, get proposal({}) from {} failed {:?}."
                                                , self, prev_prev_block_id, proposal_ex_info.proposal, remote, err);
                                            err
                                        })?;
                                    prev_proposals.insert(proposal_ex_info.proposal, proposal);
                                }

                                self.check_block_proposal_result_state_by_app(
                                    &prev_block,
                                    &prev_proposals,
                                    &Some(prev_prev_block),
                                    remote,
                                )
                                .await.map_err(|err| {
                                    log::warn!("[hotstuff] local: {:?}, rebuild of prev-prev-block({:?}) failed, {:?}."
                                        , self, prev_prev_block_id, err);
                                    err
                                })?;
                            }
                            None => {
                                log::warn!("[hotstuff] local: {:?}, rebuild result-state-id({:?}) of prev-block({:?}) failed, {:?}."
                                    , self, result_state_id, prev_block.block_id(), err);
                                return Err(err);
                            }
                        }
                    }
                }
                result_state_id.clone()
            }
            None => None,
        };

        for proposal_exe_info in block.proposals() {
            // 去重
            if let Some(prev_block_id) = block.prev_block_id() {
                let is_already_finished =  self.store
                    .is_proposal_finished(&proposal_exe_info.proposal, prev_block_id)
                    .await.map_err(|err| {
                        log::warn!("[hotstuff] local: {:?}, check proposal {:?} in block {:?} with prev-block {:?} duplicate failed, {:?}."
                            , self, proposal_exe_info.proposal, block.block_id(), prev_block_id, err);
                        err
                    })?;

                if is_already_finished {
                    log::warn!("[hotstuff] local: {:?}, proposal {:?} in block {:?} with prev-block {:?} has finished before."
                            , self, proposal_exe_info.proposal, block.block_id(), prev_block_id);

                    return Err(BuckyError::new(
                        BuckyErrorCode::ErrorState,
                        "duplicate proposal",
                    ));
                }
            }

            let proposal = proposals.get(&proposal_exe_info.proposal).unwrap();
            let receipt = match proposal_exe_info.receipt.as_ref() {
                Some(receipt) => {
                    let (receipt, _) = NONObjectInfo::raw_decode(receipt.as_slice()).map_err(|err| {
                        log::warn!("[hotstuff] local: {:?}, proposal {:?} in block {:?} decode receipt failed {:?}."
                            , self, proposal_exe_info.proposal, block.block_id(), err);

                        err
                    })?;

                    Some(receipt)
                }
                None => None,
            };

            let exe_result = ExecuteResult {
                result_state_id: proposal_exe_info.result_state,
                receipt,
                context: proposal_exe_info.context.clone(),
            };

            self
                .event_notifier
                .on_verify(proposal.clone(), prev_state_id, &exe_result)
                .await.map_err(|err| {
                    log::warn!("[hotstuff] local: {:?}, proposal {:?} in block {:?} verify by app failed {:?}."
                        , self, proposal_exe_info.proposal, block.block_id(), err);
                    err
                })?;

            log::debug!(
                "[hotstuff] local: {:?}, block verify ok by app, proposal: {}, prev_state: {:?}/{:?}, expect-result: {:?}/{:?}",
                self,
                proposal_exe_info.proposal,
                prev_state_id, prev_block.as_ref().map(|b| b.block_id()),
                proposal_exe_info.result_state,
                block.block_id()
            );

            prev_state_id = proposal_exe_info.result_state;
        }

        assert_eq!(
            &prev_state_id,
            block.result_state_id(),
            "the result state is unmatched"
        );

        Ok(())
    }

    async fn get_leader_owner(
        &self,
        group_shell_id: Option<&ObjectId>,
        round: u64,
        remote: Option<&ObjectId>,
    ) -> BuckyResult<ObjectId> {
        let leader = self
            .committee
            .get_leader(group_shell_id, round, remote)
            .await.map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, get leader from group {:?} with round {} failed, {:?}.",
                    self,
                    group_shell_id, round,
                    err
                );

                err
            })?;

        let leader_owner = self
            .non_driver
            .get_device(&leader)
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, get leader by id {:?} failed, {:?}.",
                    self,
                    leader,
                    err
                );

                err
            })?
            .desc()
            .owner()
            .clone();

        match leader_owner {
            Some(owner) => Ok(owner),
            None => {
                log::warn!(
                    "[hotstuff] local: {:?}, a owner must be set to the device {}",
                    self,
                    leader
                );
                Err(BuckyError::new(
                    BuckyErrorCode::InvalidTarget,
                    "no owner for device",
                ))
            }
        }
    }

    async fn check_block_linked(
        &mut self,
        block: &GroupConsensusBlock,
        remote: ObjectId,
    ) -> Result<Option<GroupConsensusBlock>, BuckyResult<()>> {
        match self
            .store
            .block_linked(block)
            .await
            .map_err(|err| Err(err))?
        {
            crate::storage::BlockLinkState::Expired => {
                log::warn!(
                    "[hotstuff] local: {:?}, receive block({}) expired.",
                    self,
                    block.block_id()
                );
                Err(Err(BuckyError::new(BuckyErrorCode::Ignored, "expired")))
            }
            crate::storage::BlockLinkState::Duplicate => {
                log::warn!(
                    "[hotstuff] local: {:?}, receive duplicate block({}).",
                    self,
                    block.block_id()
                );
                Err(Err(BuckyError::new(
                    BuckyErrorCode::AlreadyExists,
                    "duplicate block",
                )))
            }
            crate::storage::BlockLinkState::Link(prev_block) => {
                log::debug!(
                    "[hotstuff] local: {:?}, receive in-order block({}), height: {}.",
                    self,
                    block.block_id(),
                    block.height()
                );

                // 顺序连接状态
                Self::check_empty_block_result_state_with_prev(block, &prev_block)
                    .map_err(|err| Err(err))?;
                Ok(prev_block)
            }
            crate::storage::BlockLinkState::Pending => {
                log::warn!(
                    "[hotstuff] local: {:?}, receive out-order block({}), expect height: {}, get height: {}.",
                    self,
                    block.block_id(),
                    self.store.header_height() + 3,
                    block.height()
                );

                self.sync_to_block(block, remote).await;

                Err(Ok(()))
            }
            crate::storage::BlockLinkState::InvalidBranch => {
                log::warn!(
                    "[hotstuff] local: {:?}, receive block({}) in invalid branch.",
                    self,
                    block.block_id()
                );
                Err(Err(BuckyError::new(
                    BuckyErrorCode::Conflict,
                    "conflict branch",
                )))
            }
        }
    }

    async fn process_block(
        &mut self,
        block: &GroupConsensusBlock,
        remote: ObjectId,
        proposals: &HashMap<ObjectId, GroupProposal>,
    ) -> BuckyResult<()> {
        // all blocks should be verified ok in this function

        log::debug!("[hotstuff] local: {:?}, process_block: {:?}/{:?}/{:?}, prev: {:?}/{:?}, owner: {:?}, remote: {:?},",
            self,
            block.block_id(), block.round(), block.height(),
            block.prev_block_id(), block.qc().as_ref().map_or(0, |qc| qc.round),
            block.owner(), remote);

        if block.height() <= self.store.header_height() {
            log::warn!(
                "[hotstuff] local: {:?}, handle_block: {:?}/{:?}/{:?} ignored for expired.",
                self,
                block.block_id(),
                block.round(),
                block.height(),
            );

            return Err(BuckyError::new(BuckyErrorCode::Expired, "block expired"));
        }

        if let Err(err) = self.non_driver.put_block(block).await {
            if err.code() != BuckyErrorCode::AlreadyExists
                && err.code() != BuckyErrorCode::NotChange
            {
                log::warn!(
                    "[hotstuff] local: {:?}, put new block {:?}/{}/{} to noc",
                    self,
                    block.block_id(),
                    block.height(),
                    block.round()
                );

                return Err(err);
            }
        }

        log::info!(
            "[hotstuff] local: {:?}, will push new block {:?}/{}/{} to storage",
            self,
            block.block_id(),
            block.height(),
            block.round()
        );

        let debug_identify = self.debug_identify();
        let new_header_block = self.store.push_block(block.clone()).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, push verified block {:?} to storage failed {:?}",
                debug_identify,
                block.block_id(),
                err
            );

            err
        })?;

        if let Some((header_block, old_header_block, _discard_blocks)) = new_header_block {
            let header_block = header_block.clone();
            self.on_new_block_commit(&header_block, &old_header_block, block)
                .await;
        }

        match self.vote_mgr.add_voting_block(block).await {
            VoteThresholded::QC(qc) => {
                log::debug!(
                    "[hotstuff] local: {:?}, the qc of block {:?} has received before",
                    self,
                    block.block_id()
                );
                return self.process_block_qc(qc, block, &remote).await;
            }
            VoteThresholded::None => {}
        }

        log::debug!(
            "[hotstuff] local: {:?}, process_block-step4 {:?}",
            self,
            block.block_id()
        );

        if block.round() != self.round {
            log::debug!(
                "[hotstuff] local: {:?}, not my round {}, expect {}",
                self,
                block.round(),
                self.round
            );
            // 不是我的投票round
            return Ok(());
        }

        if let Some(vote) = self.make_vote(block, proposals, &remote).await {
            log::info!(
                "[hotstuff] local: {:?}, vote to block {}, round: {}",
                self,
                block.block_id(),
                block.round()
            );

            let next_leader = self
                .committee
                .get_leader(None, self.round + 1, None)
                .await
                .map_err(|err| {
                    log::warn!(
                        "[hotstuff] local: {:?}, get next leader in round {} failed {:?}",
                        self,
                        self.round + 1,
                        err
                    );

                    err
                })?;

            if self.local_device_id == next_leader {
                self.handle_vote(&vote, Some(block), self.local_device_id)
                    .await?;
            } else {
                self.network_sender
                    .post_message(
                        HotstuffMessage::BlockVote(vote),
                        self.rpath.clone(),
                        &next_leader,
                    )
                    .await;
            }
        }

        Ok(())
    }

    async fn on_new_block_commit(
        &mut self,
        new_header_block: &GroupConsensusBlock,
        old_header_block: &Option<GroupConsensusBlock>,
        qc_qc_block: &GroupConsensusBlock,
    ) {
        log::info!(
            "[hotstuff] local: {:?}, new header-block {:?} committed, old: {:?}, qc-qc: {}",
            self,
            new_header_block.block_id(),
            old_header_block.as_ref().map(|b| b.block_id()),
            qc_qc_block.block_id()
        );

        if new_header_block.height() <= self.max_quorum_height - 2 {
            log::info!(
                "[hotstuff] local: {:?}, new header-block {:?} committed, old: {:?}, qc-qc: {}, ignore notify history block({}/{})",
                self, new_header_block.block_id(), old_header_block.as_ref().map(|b| b.block_id()), qc_qc_block.block_id(), new_header_block.height(), self.max_quorum_height
            );
            return;
        }

        // Only the proposals included in the submitted blocks are cleaned up here
        // The proposal contained in the block to be submitted that has already been executed will be deduplicated during the next packaging
        if let Err(err) = self.cleanup_proposal(new_header_block).await {
            log::warn!("[hotstuff] local: {:?}, new header-block {:?} committed, clean finished proposal failed, and will clear them again when new block generated. err={:?}", self, new_header_block.block_id(), err);
        }

        log::debug!(
            "[hotstuff] local: {:?}, on_new_block_commit-step1 {:?}",
            self,
            qc_qc_block.block_id()
        );

        let (_, qc_block) = self
            .store
            .pre_commits()
            .iter()
            .next()
            .expect("the pre-commit block must exist.");

        if let Err(err) = self
            .notify_block_committed(new_header_block, old_header_block, qc_block)
            .await
        {
            log::warn!("[hotstuff] local: {:?}, new header-block {:?} committed, notify the members failed, and then members should sync the state. err={:?}", self, new_header_block.block_id(), err);
        }

        log::debug!(
            "[hotstuff] local: {:?}, on_new_block_commit-step2 {:?}",
            self,
            qc_qc_block.block_id()
        );

        // notify by the block generator
        if &self.local_id == new_header_block.owner() {
            // push to member
            self.state_pusher
                .notify_block_commit(new_header_block.clone(), qc_block.clone())
                .await;

            // reply
            let futs = new_header_block.proposals().iter().map(|proposal_info| {
                let receipt = match proposal_info.receipt.as_ref() {
                    Some(receipt) => {
                        NONObjectInfo::raw_decode(receipt.as_slice()).map(|(receipt, remain)| {
                            assert_eq!(remain.len(), 0);
                            Some(receipt)
                        })
                    }
                    None => Ok(None),
                };
                self.proposal_result_notifier
                    .reply(&proposal_info.proposal, receipt)
            });

            futures::future::join_all(futs).await;
        }

        log::debug!(
            "[hotstuff] local: {:?}, on_new_block_commit-step3 {:?}",
            self,
            qc_qc_block.block_id()
        );
    }

    async fn notify_block_committed(
        &self,
        new_header: &GroupConsensusBlock,
        old_header_block: &Option<GroupConsensusBlock>,
        _qc_block: &GroupConsensusBlock,
    ) -> BuckyResult<()> {
        assert_eq!(
            new_header.prev_block_id(),
            old_header_block.as_ref().map(|b| b.block_id().object_id())
        );

        if let Some(result_state_id) = new_header.result_state_id() {
            self.make_sure_result_state(result_state_id, &[new_header.owner()])
                .await?;
        }

        let prev_state_id = match old_header_block.as_ref() {
            Some(old_header_block) => {
                let result_state_id = old_header_block.result_state_id();
                if let Some(result_state_id) = result_state_id {
                    self.make_sure_result_state(result_state_id, &[old_header_block.owner()])
                        .await?;
                }
                result_state_id.clone()
            }
            None => None,
        };

        self.event_notifier
            .on_commited(prev_state_id, new_header.clone())
            .await;

        Ok(())
    }

    async fn process_qc(&mut self, qc: &Option<HotstuffBlockQC>) {
        let qc_round = qc.as_ref().map_or(0, |qc| qc.round);

        log::debug!(
            "[hotstuff] local: {:?}, process_qc round {}",
            self,
            qc_round
        );

        self.update_max_quorum_round(qc_round);
        self.advance_round(qc_round).await;
        self.update_high_qc(qc);
    }

    async fn advance_round(&mut self, round: u64) {
        if round < self.round {
            log::debug!(
                "[hotstuff] local: {:?}, round {} timeout expect {}",
                self,
                round,
                self.round
            );
            return;
        }

        log::info!(
            "[hotstuff] local: {:?}, update round from {} to {}",
            self,
            self.round,
            round + 1
        );

        self.timer.reset(GROUP_DEFAULT_CONSENSUS_INTERVAL);
        self.round = round + 1;
        self.vote_mgr.cleanup(self.round);
        self.tc = None;
    }

    fn update_high_qc(&mut self, qc: &Option<HotstuffBlockQC>) {
        let to_high_round = qc.as_ref().map_or(0, |qc| qc.round);
        let cur_high_round = self.high_qc.as_ref().map_or(0, |qc| qc.round);
        if to_high_round > cur_high_round {
            self.high_qc = qc.clone();

            log::info!(
                "[hotstuff] local: {:?}, update high-qc from {} to {}",
                self,
                cur_high_round,
                to_high_round
            );
        }
    }

    fn update_max_quorum_round(&mut self, quorum_round: u64) {
        if quorum_round > self.max_quorum_round {
            self.max_quorum_round = quorum_round;
        }
    }

    fn update_max_quorum_height(&mut self, quorum_height: u64) {
        if quorum_height > self.max_quorum_height {
            self.max_quorum_height = quorum_height;
        }
    }

    async fn cleanup_proposal(&mut self, commited_block: &GroupConsensusBlock) -> BuckyResult<()> {
        let proposals = commited_block
            .proposals()
            .iter()
            .map(|proposal| proposal.proposal)
            .collect::<Vec<_>>();

        log::debug!(
            "[hotstuff] local: {:?}, remove proposals: {:?}",
            self,
            proposals.len()
        );

        self.proposal_consumer.remove_proposals(proposals).await
    }

    async fn notify_proposal_err(&self, proposal: &GroupProposal, err: BuckyError) {
        log::debug!(
            "[hotstuff] local: {:?}, proposal {} failed {:?}",
            self,
            proposal.desc().object_id(),
            err
        );

        self.proposal_result_notifier
            .reply(&proposal.desc().object_id(), Err(err.clone()))
            .await;

        self.state_pusher
            .notify_proposal_err(proposal.clone(), err)
            .await;
    }

    async fn make_vote(
        &mut self,
        block: &GroupConsensusBlock,
        mut proposals: &HashMap<ObjectId, GroupProposal>,
        remote: &ObjectId,
    ) -> Option<HotstuffBlockQCVote> {
        log::debug!(
            "[hotstuff] local: {:?} make vote {} step 0",
            self,
            block.block_id()
        );

        if block.round() <= self.store.last_vote_round() {
            log::debug!("[hotstuff] local: {:?}, make vote ignore for timeouted block {}/{}, last vote roud: {}",
                self, block.block_id(), block.round(), self.store.last_vote_round());

            return None;
        }

        let mut only_rebuild_result_state = false;
        if self.max_quorum_height >= self.store.max_height() {
            if let Some(result_state_id) = block.result_state_id() {
                // `make_sure_result_state` will rebuild result-state by downloading from remote.
                if self
                    .make_sure_result_state(result_state_id, &[block.owner(), remote])
                    .await
                    .is_err()
                {
                    // download from remote failed, we need to calcute the result-state by the DEC.on_verify
                    only_rebuild_result_state = true;
                }
            }

            if !only_rebuild_result_state {
                log::debug!("[hotstuff] local: {:?}, make vote ignore for the block {}/{} has enough votes {}/{}.",
                    self, block.block_id(), block.round(), self.max_quorum_round, self.round);

                return None;
            }
        }

        let prev_block = match block.prev_block_id() {
            Some(prev_block_id) => match self.store.find_block_in_cache(prev_block_id) {
                Ok(block) => Some(block),
                Err(_) => {
                    log::warn!("[hotstuff] local: {:?}, make vote to block {} ignore for prev-block {:?} is invalid",
                        self,
                        block.block_id(),
                        block.prev_block_id()
                    );

                    return None;
                }
            },
            None => None,
        };

        // select the highest branch to vote.
        if block.height() != self.store.max_height() {
            log::warn!(
                "[hotstuff] local: {:?}, make vote to block {} ignore for higher({}!={}) branch.",
                self,
                block.block_id(),
                block.height(),
                self.store.max_height(),
            );

            return None;
        }

        // `round` must be increased one by one
        let qc_round = block.qc().as_ref().map_or(0, |qc| qc.round);
        if qc_round != prev_block.as_ref().map_or(0, |p| p.round()) {
            log::warn!("[hotstuff] local: {:?}, make vote to block {} ignore for qc-round({}) is unmatch with prev-block({}/{:?})",
                self,
                block.block_id(),
                qc_round,
                prev_block.as_ref().map_or(0, |p| p.round()),
                block.prev_block_id()
            );
            return None;
        }

        let is_valid_round = if block.round() == qc_round + 1 {
            true
        } else if let Some(tc) = block.tc() {
            block.round() == tc.round + 1
            // && qc_round
            //     >= tc.votes.iter().map(|v| v.high_qc_round).max().unwrap()
            // maybe some block timeout happened, the leaders has the larger round QC, but not broadcast to others
        } else {
            false
        };

        if !is_valid_round {
            log::warn!("[hotstuff] local: {:?}, make vote to block {} ignore for invalid round {}, qc-round {}, tc-round {:?}",
                self,
                block.block_id(),
                block.round(), qc_round,
                block.tc().as_ref().map_or((0, 0), |tc| {
                    let qc_round = tc.votes.iter().map(|v| v.high_qc_round).max().unwrap();
                    (tc.round, qc_round)
                }));

            return None;
        }

        log::debug!(
            "[hotstuff] local: {:?} make vote {} step 1",
            self,
            block.block_id()
        );

        if !only_rebuild_result_state {
            match self.check_group_is_latest(block.group_shell_id()).await {
                Ok(is_latest) if is_latest => {}
                _ => {
                    log::warn!("[hotstuff] local: {:?}, make vote to block {} ignore for the group is not latest",
                        self,
                        block.block_id());

                    return None;
                }
            }
        }

        log::debug!(
            "[hotstuff] local: {:?} make vote {} step 2",
            self,
            block.block_id()
        );

        let mut proposal_temp: HashMap<ObjectId, GroupProposal> = HashMap::new();
        if proposals.len() == 0 && block.proposals().len() > 0 {
            match self
                .non_driver
                .load_all_proposals_for_block(block, &mut proposal_temp)
                .await
            {
                Ok(_) => proposals = &proposal_temp,
                Err(err) => {
                    log::warn!("[hotstuff] local: {:?}, make vote to block {} ignore for load proposals failed {:?}",
                        self,
                        block.block_id(),
                        err
                    );
                    return None;
                }
            }
        } else {
            assert_eq!(proposals.len(), block.proposals().len());
        }

        log::debug!(
            "[hotstuff] local: {:?} make vote {} step 3",
            self,
            block.block_id()
        );

        // 时间和本地误差太大，不签名，打包的proposal时间和block时间差距太大，也不签名
        if !only_rebuild_result_state
            && !Self::check_timestamp_precision(block, prev_block.as_ref(), proposals)
        {
            log::warn!(
                "[hotstuff] local: {:?}, make vote to block {} ignore for timestamp mismatch",
                self,
                block.block_id(),
            );
            return None;
        }

        if !only_rebuild_result_state && proposals.len() != block.proposals().len() {
            let mut dup_proposals = block.proposals().clone();
            dup_proposals.sort_unstable_by_key(|p| p.proposal);
            log::warn!(
                "[hotstuff] local: {:?}, make vote to block {} ignore for proposals {:?} duplicate",
                self,
                block.block_id(),
                dup_proposals
                    .iter()
                    .group_by(|p| p.proposal)
                    .into_iter()
                    .map(|g| (g.0, g.1.count()))
                    .filter(|g| g.1 > 1)
                    .map(|g| g.0)
                    .collect_vec()
            );
            return None;
        }

        log::debug!(
            "[hotstuff] local: {:?} make vote {} step 4",
            self,
            block.block_id()
        );

        // 1. verify the results for proposals by DecApp
        // 2. rebuild the result-state in on_verify by DecApp
        if let Err(err) = self
            .check_block_proposal_result_state_by_app(block, &proposals, &prev_block, remote)
            .await
        {
            log::warn!(
                "[hotstuff] local: {:?}, make vote to block {} ignore for app verify failed {:?}",
                self,
                block.block_id(),
                err
            );
            return None;
        }

        if only_rebuild_result_state {
            log::debug!("[hotstuff] local: {:?}, make vote ignore for the block {}/{} has enough votes {}/{} rebuild only.",
                self, block.block_id(), block.round(), self.max_quorum_round, self.round);
            return None;
        }

        log::debug!(
            "[hotstuff] local: {:?}, make-vote before sign {}, round: {}",
            self,
            block.block_id(),
            block.round()
        );

        let vote = match HotstuffBlockQCVote::new(block, self.local_device_id, &self.signer).await {
            Ok(vote) => {
                log::debug!(
                    "[hotstuff] local: {:?}, make-vote after sign {}, round: {}",
                    self,
                    block.block_id(),
                    block.round()
                );

                vote
            }
            Err(e) => {
                log::warn!(
                    "[hotstuff] local: {:?}, signature for block-vote failed, block: {}, err: {}",
                    self,
                    block.block_id(),
                    e
                );
                return None;
            }
        };

        if let Err(err) = self.store.set_last_vote_round(block.round()).await {
            log::warn!("[hotstuff] local: {:?}, make vote to block {} ignore for update last-vote-round failed {:?}",
                self,
                block.block_id(),
                err
            );
            return None;
        }

        log::debug!(
            "[hotstuff] local: {:?} make vote {} step 5",
            self,
            block.block_id()
        );

        Some(vote)
    }

    fn check_timestamp_precision(
        block: &GroupConsensusBlock,
        prev_block: Option<&GroupConsensusBlock>,
        proposals: &HashMap<ObjectId, GroupProposal>,
    ) -> bool {
        let now = SystemTime::now();
        let block_timestamp = bucky_time_to_system_time(block.named_object().desc().create_time());
        if Self::calc_time_delta(now, block_timestamp) > TIME_PRECISION {
            log::warn!(
                "[hotstuff] block {} check timestamp {:?} failed with now {:?}",
                block.block_id(),
                block_timestamp,
                now
            );

            false
        } else {
            if let Some(prev_block) = prev_block {
                let prev_block_time =
                    bucky_time_to_system_time(prev_block.named_object().desc().create_time());
                if let Ok(duration) = prev_block_time.duration_since(block_timestamp) {
                    if duration > TIME_PRECISION {
                        log::warn!(
                            "[hotstuff] block {} check timestamp {:?} failed with prev-block {:?}",
                            block.block_id(),
                            block_timestamp,
                            prev_block_time
                        );
                        return false;
                    }
                }
            }

            for proposal in block.proposals() {
                let proposal_id = proposal.proposal;
                let proposal = proposals
                    .get(&proposal_id)
                    .expect("should load all proposals");
                let proposal_timestamp = bucky_time_to_system_time(proposal.desc().create_time());
                if Self::calc_time_delta(block_timestamp, proposal_timestamp) > TIME_PRECISION {
                    log::warn!(
                        "[hotstuff] block {} check timestamp {:?} failed with proposal({:?}) {:?}",
                        block.block_id(),
                        block_timestamp,
                        proposal_id,
                        proposal_timestamp
                    );
                    return false;
                }
            }
            true
        }
    }

    fn calc_time_delta(t1: SystemTime, t2: SystemTime) -> Duration {
        t1.duration_since(t2).or(t2.duration_since(t1)).unwrap()
    }

    async fn handle_vote(
        &mut self,
        vote: &HotstuffBlockQCVote,
        prev_block: Option<&GroupConsensusBlock>,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        log::debug!("[hotstuff] local: {:?}, handle_vote: {:?}/{:?}, prev: {:?}, voter: {:?}, remote: {:?},",
            self,
            vote.block_id, vote.round,
            vote.prev_block_id,
            vote.voter, remote);

        if vote.round < self.round {
            log::warn!(
                "[hotstuff] local: {:?}, receive timeout vote({}/{}/{:?}), local-round: {}",
                self,
                vote.block_id,
                vote.round,
                vote.prev_block_id,
                self.round
            );
            return Ok(());
        }

        self.committee.verify_vote(vote).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, verify vote({}/{}/{:?}) failed {:?}",
                self,
                vote.block_id,
                vote.round,
                vote.prev_block_id,
                err
            );
            err
        })?;

        let prev_block = match prev_block {
            Some(b) => Some(b.clone()),
            None => self
                .store
                .find_block_in_cache(&vote.block_id)
                .map_or(None, |b| Some(b)),
        };

        let is_prev_none = prev_block.is_none();
        let qc = self
            .vote_mgr
            .add_vote(vote.clone(), prev_block)
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, add vote({}/{}/{:?}) prev-block: {} failed {:?}",
                    self,
                    vote.block_id,
                    vote.round,
                    vote.prev_block_id,
                    if is_prev_none { "None" } else { "Some" },
                    err
                );
                err
            })?;

        if let Some((qc, block)) = qc {
            log::info!(
                "[hotstuff] local: {:?}, vote({}/{}/{:?}) prev-block: {} qc",
                self,
                vote.block_id,
                vote.round,
                vote.prev_block_id,
                if is_prev_none { "None" } else { "Some" }
            );

            self.process_block_qc(qc, &block, &remote).await?;
        } else if vote.round > self.round && is_prev_none {
            self.fetch_block(&vote.block_id, remote).await?;
        }
        Ok(())
    }

    async fn process_block_qc(
        &mut self,
        qc: HotstuffBlockQC,
        prev_block: &GroupConsensusBlock,
        remote: &ObjectId,
    ) -> BuckyResult<()> {
        let qc_block_id = qc.block_id;
        let qc_round = qc.round;
        let qc_prev_block_id = qc.prev_block_id;

        log::debug!("[hotstuff] local: {:?},  save-qc round {}", self, qc_round);

        self.store.save_qc(&qc).await?;

        self.process_qc(&Some(qc)).await;

        self.update_max_quorum_height(prev_block.height());

        let new_leader = self.committee.get_leader(None, self.round, Some(remote)).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, get leader for vote-qc({}/{}/{:?}) with round {} failed {:?}",
                self,
                qc_block_id, qc_round, qc_prev_block_id,
                self.round,
                err
            );
            err
        })?;

        if self.local_device_id == new_leader {
            if let Err(err) = self.generate_block(self.with_tc()).await {
                log::warn!("[hotstuff] local: {:?}, block qc, but my new block generate failed, will wait to timeout. block: {}, err={:?}", self, qc_block_id, err);
            }
        }
        Ok(())
    }

    async fn handle_timeout(
        &mut self,
        timeout: &HotstuffTimeoutVote,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        log::debug!(
            "[hotstuff] local: {:?}, handle_timeout: {:?}, qc: {:?}, voter: {:?}, remote: {:?},",
            self,
            timeout.round,
            timeout.high_qc.as_ref().map(|qc| format!(
                "{:?}/{:?}/{:?}/{:?}",
                qc.block_id,
                qc.round,
                qc.prev_block_id,
                qc.votes
                    .iter()
                    .map(|v| v.voter.to_string())
                    .collect::<Vec<String>>()
            )),
            timeout.voter,
            remote
        );

        if timeout.round < self.round {
            if let Some(tc) = self.tc.as_ref() {
                // if there is a timeout-qc, notify the remote to advance the round
                if tc.round + 1 == self.round {
                    self.network_sender
                        .post_message(
                            HotstuffMessage::Timeout(tc.clone()),
                            self.rpath.clone(),
                            &remote,
                        )
                        .await;
                }
            }
            return Ok(());
        }

        let high_qc_round = timeout.high_qc.as_ref().map_or(0, |qc| qc.round);
        if high_qc_round >= timeout.round {
            log::warn!(
                "[hotstuff] local: {:?}, handle_timeout: {:?}, ignore for high-qc(round={}) invalid",
                self,
                timeout.round,
                high_qc_round
            );
            return Ok(());
        }

        match self.check_group_is_latest(&timeout.group_shell_id).await {
            Ok(is) if is => {}
            _ => {
                log::warn!(
                    "[hotstuff] local: {:?}, handle_timeout: {:?}, ignore for is not latest group.",
                    self,
                    timeout.round,
                );
                return Ok(());
            }
        }

        let block = match timeout.high_qc.as_ref() {
            Some(qc) => match self.store.find_block_in_cache(&qc.block_id) {
                Ok(block) => Some(block),
                Err(err) => {
                    log::warn!(
                        "[hotstuff] local: {:?}, handle_timeout: {:?}, find qc-block {} failed {:?}",
                        self,
                        timeout.round,
                        qc.block_id,
                        err
                    );

                    if let Err(err) = self.fetch_block(&qc.block_id, remote).await {
                        log::warn!("[hotstuff] local: {:?}, handle_timeout: {:?}, fetch the prev block failed, will wait it, err: {:?}", self, timeout.round, err);
                    }
                    return Ok(());
                }
            },
            None => None,
        };

        self.committee
            .verify_timeout(timeout, block.as_ref())
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, handle_timeout: {:?}, verify failed {:?}",
                    self,
                    timeout.round,
                    err
                );

                err
            })?;

        self.process_qc(&timeout.high_qc).await;

        let tc = self
            .vote_mgr
            .add_timeout(timeout.clone())
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, handle_timeout: {:?}, check tc failed {:?}",
                    self,
                    timeout.round,
                    err
                );
                err
            })?;

        if let Some(tc) = tc {
            self.process_timeout_qc(tc, &remote).await?;
        }
        Ok(())
    }

    async fn process_timeout_qc(
        &mut self,
        tc: HotstuffTimeout,
        remote: &ObjectId,
    ) -> BuckyResult<()> {
        log::debug!(
            "[hotstuff] local: {:?}, process_timeout_qc: {:?}, voter: {:?}.",
            self,
            tc.round,
            tc.votes
                .iter()
                .map(|vote| format!("{:?}/{:?}", vote.high_qc_round, vote.voter,))
                .collect::<Vec<String>>(),
        );

        let quorum_round = tc.round;
        self.update_max_quorum_round(quorum_round);

        self.store
            .save_tc(
                &tc,
                tc.group_shell_id
                    .expect("group-shell-id should not be None."),
            )
            .await?;

        self.advance_round(tc.round).await;
        self.tc = Some(tc.clone());

        log::debug!("[hotstuff] local: {:?},  save-tc round {}", self, tc.round);

        let new_leader = self
            .committee
            .get_leader(None, self.round, Some(remote))
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, process_timeout_qc: {:?}, get new leader failed {:?}",
                    self,
                    tc.round,
                    err
                );

                err
            })?;
        if self.local_device_id == new_leader {
            if let Err(err) = self.generate_block(Some(tc.clone())).await {
                log::warn!("[hotstuff] local: {:?}, process_timeout_qc: {:?}, generate new block failed. err: {:?}", self, tc.round, err);
            }
        } else {
            let (latest_group, _latest_shell_id) = self.shell_mgr.group();

            self.broadcast(HotstuffMessage::Timeout(tc), &latest_group)
        }

        Ok(())
    }

    async fn handle_tc(&mut self, tc: &HotstuffTimeout, remote: ObjectId) -> BuckyResult<()> {
        let max_high_qc = tc
            .votes
            .iter()
            .max_by(|high_qc_l, high_qc_r| high_qc_l.high_qc_round.cmp(&high_qc_r.high_qc_round));

        log::debug!(
            "[hotstuff] local: {:?}, handle_tc: {:?}, voter: {:?}, remote: {:?}, max-qc: {:?}",
            self,
            tc.round,
            tc.votes
                .iter()
                .map(|vote| format!("{:?}/{:?}", vote.high_qc_round, vote.voter,))
                .collect::<Vec<String>>(),
            remote,
            max_high_qc.as_ref().map(|qc| qc.high_qc_round)
        );

        let max_high_qc = match max_high_qc {
            Some(max_high_qc) => max_high_qc,
            None => return Ok(()),
        };

        if tc.round < self.round {
            log::warn!(
                "[hotstuff] local: {:?}, handle_tc: {:?} ignore for round timeout",
                self,
                tc.round,
            );
            return Ok(());
        }

        if max_high_qc.high_qc_round >= tc.round {
            log::warn!(
                "[hotstuff] local: {:?}, handle_tc: {:?} ignore for high-qc round {} invalid",
                self,
                tc.round,
                max_high_qc.high_qc_round
            );

            return Ok(());
        }

        let group_shell_id = match tc.group_shell_id.as_ref() {
            Some(group_shell_id) => group_shell_id.clone(),
            None => {
                log::warn!(
                    "[hotstuff] local: {:?}, handle_tc: {:?} ignore for group-shell-id is None.",
                    self,
                    tc.round
                );
                return Ok(());
            }
        };

        if max_high_qc.high_qc_round > 0 {
            if let Err(err) = self
                .store
                .find_block_in_cache_by_round(max_high_qc.high_qc_round)
            {
                log::warn!(
                    "[hotstuff] local: {:?}, handle_tc: {:?} find prev-block by round {} failed {:?}",
                    self,
                    tc.round, max_high_qc.high_qc_round,
                    err
                );

                // 同步前序block
                let max_round_block = self.store.block_with_max_round();
                self.synchronizer.sync_with_round(
                    max_round_block.map_or(1, |block| block.height() + 1),
                    max_high_qc.high_qc_round,
                    remote,
                );
            };
        }

        self.committee
            .verify_tc(tc, &group_shell_id)
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, handle_tc: {:?} verify tc failed {:?}",
                    self,
                    tc.round,
                    err
                );
                err
            })?;

        log::debug!("[hotstuff] local: {:?},  save-tc round {}", self, tc.round);

        let quorum_round = tc.round;
        self.update_max_quorum_round(quorum_round);

        self.store.save_tc(&tc, group_shell_id).await?;

        self.advance_round(tc.round).await;
        self.tc = Some(tc.clone());

        let new_leader = self
            .committee
            .get_leader(None, self.round, Some(&remote))
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, handle_tc: {:?} get new leader failed {:?}",
                    self,
                    tc.round,
                    err
                );
                err
            })?;

        if self.local_device_id == new_leader {
            if let Err(err) = self.generate_block(Some(tc.clone())).await {
                log::warn!(
                    "[hotstuff] local: {:?}, handle_tc: {:?}, generate new block failed. err: {:?}",
                    self,
                    tc.round,
                    err
                );
            }
        }
        Ok(())
    }

    async fn local_timeout_round(&mut self) -> BuckyResult<()> {
        log::debug!(
            "[hotstuff] local: {:?}, local_timeout_round, max_quorum_height: {}, max_height: {}",
            self,
            self.max_quorum_height,
            self.store.max_height()
        );

        if self.is_synchronizing() {
            log::info!("[hotstuff] local: {:?}, local_timeout_round, is synchronizing, ignore the timeout. max_quorum_height: {}, max_height: {}", self, self.max_quorum_height, self.store.max_height());
            return Ok(());
        }

        let latest_group = match self
            .shell_mgr
            .get_group(self.rpath.group_id(), None, None)
            .await
        {
            Ok(group) => {
                self.timer.reset(GROUP_DEFAULT_CONSENSUS_INTERVAL);
                group
            }
            Err(err) => {
                log::warn!(
                    "[hotstuff] local: {:?}, local_timeout_round get latest group failed {:?}",
                    self,
                    err
                );

                self.timer.reset(HOTSTUFF_TIMEOUT_DEFAULT);
                return Err(err);
            }
        };

        log::debug!(
            "[hotstuff] local: {:?}, local_timeout_round, latest group got.",
            self,
        );

        let latest_group_shell = latest_group.to_shell();
        let latest_group_shell_id = latest_group_shell.shell_id();
        let timeout = HotstuffTimeoutVote::new(
            latest_group_shell_id,
            self.high_qc.clone(),
            self.round,
            self.local_device_id,
            &self.signer,
        )
        .await
        .map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, local_timeout_round create new timeout-vote failed {:?}",
                self,
                err
            );
            err
        })?;

        log::debug!(
            "[hotstuff] local: {:?}, local_timeout_round, vote for timeout created.",
            self,
        );

        let ret = self.store.set_last_vote_round(self.round).await;
        log::debug!("[hotstuff] local: {:?}, local_timeout_round, round last vote is stored. round = {}, result: {:?}", self, self.round, ret);
        ret?;

        self.broadcast(HotstuffMessage::TimeoutVote(timeout.clone()), &latest_group);

        log::debug!(
            "[hotstuff] local: {:?}, local_timeout_round, broadcast.",
            self,
        );

        if let Err(err) = self
            .tx_message
            .send((HotstuffMessage::TimeoutVote(timeout), self.local_device_id))
            .await
        {
            log::warn!(
                "[hotstuff] local: {:?}, local_timeout_round send message failed {:?}",
                self,
                err
            );
        }

        Ok(())
    }

    fn is_synchronizing(&self) -> bool {
        self.max_quorum_height > self.store.max_height() + BLOCK_COUNT_REST_TO_SYNC
    }

    async fn generate_block(&mut self, tc: Option<HotstuffTimeout>) -> BuckyResult<()> {
        let now = SystemTime::now();

        log::debug!(
            "[hotstuff] local: {:?}, generate_block with qc {:?} and tc {:?}, now: {:?}",
            self,
            self.high_qc.as_ref().map(|qc| format!(
                "{}/{}/{:?}",
                qc.block_id,
                qc.round,
                qc.votes.iter().map(|v| v.voter).collect::<Vec<_>>()
            )),
            tc.as_ref().map(|tc| format!(
                "{}/{:?}",
                tc.round,
                tc.votes.iter().map(|v| v.voter).collect::<Vec<_>>()
            )),
            now
        );

        let mut proposals = self
            .proposal_consumer
            .query_proposals()
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, generate_block query proposal failed {:?}",
                    self,
                    err
                );
                err
            })?;

        proposals.sort_by(|left, right| left.desc().create_time().cmp(&right.desc().create_time()));

        let prev_block = match self.high_qc.as_ref() {
            Some(qc) => {
                let prev_block = self.store.find_block_in_cache(&qc.block_id)?;
                if let Some(result_state_id) = prev_block.result_state_id() {
                    self.make_sure_result_state(result_state_id, &[prev_block.owner()])
                        .await?;
                }
                Some(prev_block)
            }
            None => None,
        };
        let latest_group = self
            .shell_mgr
            .get_group(self.rpath.group_id(), None, None)
            .await
            .map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, generate_block get latest group failed {:?}",
                    self,
                    err
                );

                err
            })?;

        let mut remove_proposals = vec![];
        // let mut dup_proposals = vec![];
        let mut time_adjust_proposals = vec![];
        let mut timeout_proposals = vec![];
        let mut executed_proposals = vec![];
        let mut failed_proposals = vec![];
        let mut result_state_id = match prev_block.as_ref() {
            Some(block) => block.result_state_id().clone(),
            None => self.store.dec_state_id().clone(),
        };

        // TODO: The time may be too long for too many proposals
        for proposal in proposals {
            let proposal_id = proposal.desc().object_id();
            if let Some(high_qc) = self.high_qc.as_ref() {
                if let Ok(is_finished) = self
                    .store
                    .is_proposal_finished(&proposal_id, &high_qc.block_id)
                    .await
                {
                    if is_finished {
                        // dup_proposals.push(proposal);
                        remove_proposals.push(proposal_id);
                        continue;
                    }
                }
            }

            let create_time = bucky_time_to_system_time(proposal.desc().create_time());
            if Self::calc_time_delta(now, create_time) > TIME_PRECISION {
                // 时间误差太大
                remove_proposals.push(proposal.desc().object_id());
                time_adjust_proposals.push(proposal);
                continue;
            }

            let ending = proposal
                .effective_ending()
                .map_or(now.checked_add(PROPOSAL_MAX_TIMEOUT).unwrap(), |ending| {
                    bucky_time_to_system_time(ending)
                });
            if now >= ending {
                remove_proposals.push(proposal.desc().object_id());
                timeout_proposals.push(proposal);
                continue;
            }

            match self
                .event_notifier
                .on_execute(proposal.clone(), result_state_id)
                .await
            {
                Ok(exe_result) => {
                    result_state_id = exe_result.result_state_id;
                    executed_proposals.push((proposal, exe_result));
                }
                Err(e) => {
                    remove_proposals.push(proposal_id);
                    failed_proposals.push((proposal, e));
                }
            };
        }

        self.notify_adjust_time_proposals(time_adjust_proposals)
            .await;
        self.notify_timeout_proposals(timeout_proposals).await;
        self.notify_failed_proposals(failed_proposals).await;
        self.remove_pending_proposals(remove_proposals).await;

        if self
            .try_wait_proposals(executed_proposals.len(), &prev_block)
            .await
        {
            log::debug!(
                "[hotstuff] local: {:?}, generate_block empty block, will ignore",
                self,
            );
            return Ok(());
        }

        let proposals_map = HashMap::from_iter(
            executed_proposals
                .iter()
                .map(|(proposal, _)| (proposal.desc().object_id(), proposal.clone())),
        );

        let block = self
            .package_block_with_proposals(
                executed_proposals,
                &latest_group,
                result_state_id,
                &prev_block,
                tc,
            )
            .await?;

        self.broadcast(HotstuffMessage::Block(block.clone()), &latest_group);
        if let Err(err) = self.tx_block_gen.send((block, proposals_map)).await {
            log::warn!(
                "[hotstuff] local: {:?}, generate_block send generated new block failed, err: {:?}.",
                self, err
            );
        }

        self.rx_proposal_waiter = None;
        Ok(())
    }

    async fn notify_adjust_time_proposals(&self, time_adjust_proposals: Vec<GroupProposal>) {
        if time_adjust_proposals.len() > 0 {
            log::warn!(
                "[hotstuff] local: {:?}, generate_block timestamp err {:?}",
                self,
                time_adjust_proposals
                    .iter()
                    .map(|proposal| {
                        let desc = proposal.desc();
                        (
                            desc.object_id(),
                            desc.owner(),
                            bucky_time_to_system_time(desc.create_time()),
                        )
                    })
                    .collect::<Vec<_>>()
            );
        }

        for proposal in time_adjust_proposals {
            // timestamp is error
            self.notify_proposal_err(
                &proposal,
                BuckyError::new(BuckyErrorCode::ErrorTimestamp, "error timestamp"),
            )
            .await;
        }
    }

    async fn notify_timeout_proposals(&self, timeout_proposals: Vec<GroupProposal>) {
        if timeout_proposals.len() > 0 {
            log::warn!(
                "[hotstuff] local: {:?}, generate_block timeout {:?}",
                self,
                timeout_proposals
                    .iter()
                    .map(|proposal| {
                        let desc = proposal.desc();
                        (
                            desc.object_id(),
                            desc.owner(),
                            bucky_time_to_system_time(desc.create_time()),
                            proposal
                                .effective_ending()
                                .as_ref()
                                .map(|ending| bucky_time_to_system_time(*ending)),
                        )
                    })
                    .collect::<Vec<_>>()
            );
        }

        for proposal in timeout_proposals {
            // has timeout
            self.notify_proposal_err(
                &proposal,
                BuckyError::new(BuckyErrorCode::Timeout, "timeout"),
            )
            .await;
        }
    }

    async fn notify_failed_proposals(&self, failed_proposals: Vec<(GroupProposal, BuckyError)>) {
        if failed_proposals.len() > 0 {
            log::warn!(
                "[hotstuff] local: {:?}, generate_block failed proposal {:?}",
                self,
                failed_proposals
                    .iter()
                    .map(|(proposal, err)| {
                        let desc = proposal.desc();
                        (desc.object_id(), desc.owner(), err.clone())
                    })
                    .collect::<Vec<_>>()
            );
        }

        for (proposal, err) in failed_proposals {
            // failed
            self.notify_proposal_err(&proposal, err).await;
        }
    }

    async fn remove_pending_proposals(&self, pending_proposals: Vec<ObjectId>) {
        if pending_proposals.len() > 0 {
            log::warn!(
                "[hotstuff] local: {:?}, generate_block finish proposal {:?}",
                self,
                pending_proposals
            );
        }

        if let Err(err) = self
            .proposal_consumer
            .remove_proposals(pending_proposals)
            .await
        {
            log::warn!(
                "[hotstuff] local: {:?}, generate_block remove finished proposals failed, err: {:?}.",
                self,
                err
            );
        }
    }

    async fn package_block_with_proposals(
        &self,
        executed_proposals: Vec<(GroupProposal, ExecuteResult)>,
        group: &Group,
        result_state_id: Option<ObjectId>,
        prev_block: &Option<GroupConsensusBlock>,
        tc: Option<HotstuffTimeout>,
    ) -> BuckyResult<GroupConsensusBlock> {
        let proposal_count = executed_proposals.len();
        let proposals_param = executed_proposals
            .into_iter()
            .map(|(proposal, exe_result)| GroupConsensusBlockProposal {
                proposal: proposal.desc().object_id(),
                result_state: exe_result.result_state_id,
                receipt: exe_result.receipt.map(|receipt| receipt.to_vec().unwrap()),
                context: exe_result.context,
            })
            .collect();

        let group_shell_id = group.to_shell().shell_id();

        let mut block = GroupConsensusBlock::create(
            self.rpath.clone(),
            proposals_param,
            result_state_id,
            prev_block.as_ref().map_or(0, |b| b.height()) + 1,
            ObjectId::default(), // TODO: meta block id
            self.round,
            group_shell_id,
            self.high_qc.clone(),
            tc,
            self.local_id,
        );

        log::info!(
            "[hotstuff] local: {:?}, generate_block new block {}/{}/{}, with proposals: {}",
            self,
            block.block_id(),
            block.height(),
            block.round(),
            proposal_count
        );

        self.sign_block(&mut block).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, generate_block new block {} sign failed {:?}",
                self,
                block.block_id(),
                err
            );

            err
        })?;

        self.non_driver.put_block(&block).await?;

        Ok(block)
    }

    async fn sign_block(&self, block: &mut GroupConsensusBlock) -> BuckyResult<()> {
        let sign_source = SignatureSource::Object(ObjectLink {
            obj_id: self.local_device_id,
            obj_owner: None,
        });

        let desc_hash = block.named_object().desc().raw_hash_value()?;
        let signature = self.signer.sign(desc_hash.as_slice(), &sign_source).await?;
        block
            .named_object_mut()
            .signs_mut()
            .set_desc_sign(signature);

        Ok(())
    }

    fn broadcast(&self, msg: HotstuffMessage, group: &Group) {
        let targets: Vec<ObjectId> = group
            .ood_list()
            .iter()
            .filter(|ood_id| **ood_id != self.local_device_id)
            .map(|ood_id| ood_id.object_id().clone())
            .collect();

        let network_sender = self.network_sender.clone();
        let rpath = self.rpath.clone();

        async_std::task::spawn(async move {
            network_sender
                .broadcast(msg, rpath.clone(), targets.as_slice())
                .await
        });
    }

    async fn try_wait_proposals(
        &mut self,
        proposal_count: usize,
        pre_block: &Option<GroupConsensusBlock>,
    ) -> bool {
        // empty block, qc only, it's unuseful when no block to qc
        let mut will_wait_proposals = false;
        if proposal_count == 0 {
            match pre_block.as_ref() {
                None => {
                    log::warn!(
                        "[hotstuff] local: {:?}, new empty block will ignore for first block is empty.",
                        self,
                    );

                    will_wait_proposals = true
                }
                Some(pre_block) => {
                    if pre_block.proposals().len() == 0 {
                        match pre_block.prev_block_id() {
                            Some(pre_pre_block_id) => {
                                let pre_pre_block = match self
                                    .store
                                    .find_block_in_cache(pre_pre_block_id)
                                {
                                    Ok(pre_pre_block) => pre_pre_block,
                                    Err(err) => {
                                        log::warn!(
                                                "[hotstuff] local: {:?}, new empty block will generate for find prev-block {} failed {:?}",
                                                self,
                                                pre_pre_block_id,
                                                err
                                            );
                                        return false;
                                    }
                                };
                                if pre_pre_block.proposals().len() == 0 {
                                    log::warn!(
                                        "[hotstuff] local: {:?}, new empty block will ignore for 2 prev-block({}/{}) is empty",
                                        self,
                                        pre_pre_block_id, pre_block.block_id()
                                    );

                                    will_wait_proposals = true;
                                }
                            }
                            None => {
                                log::warn!(
                                    "[hotstuff] local: {:?}, new empty block will ignore for prev-prev-block is None and prev-block is {}, maybe is a bug.",
                                    self,
                                    pre_block.block_id()
                                );

                                will_wait_proposals = true;
                            }
                        }
                    }
                }
            }
        }

        if will_wait_proposals {
            match self.proposal_consumer.wait_proposals().await {
                Ok(rx) => self.rx_proposal_waiter = Some((rx, self.round)),
                _ => return false,
            }
        }

        will_wait_proposals
    }

    async fn handle_proposal_waiting(&mut self) -> BuckyResult<()> {
        log::debug!("[hotstuff] local: {:?}, handle_proposal_waiting", self);

        assert_eq!(
            self.committee.get_leader(None, self.round, None).await?,
            self.local_device_id
        );

        self.generate_block(self.with_tc()).await
    }

    fn with_tc(&self) -> Option<HotstuffTimeout> {
        self.tc.as_ref().map_or(None, |tc| {
            if tc.round + 1 == self.round {
                Some(tc.clone())
            } else {
                None
            }
        })
    }

    async fn fetch_block(&mut self, block_id: &ObjectId, remote: ObjectId) -> BuckyResult<()> {
        let block = self
            .non_driver
            .get_block(block_id, Some(&remote))
            .await
            .map_err(|err| {
                log::error!(
                    "[hotstuff] local: {:?}, fetch block({}) from {} failed, err: {:?}.",
                    self,
                    block_id,
                    remote,
                    err
                );
                err
            })?;

        if let Err(err) = self.tx_message_inner.send((block, remote)).await {
            log::warn!(
                "[hotstuff] local: {:?}, fetch block({}) from {} failed when send fetched block, err: {:?}.",
                self,
                block_id,
                remote,
                err
            );
        }

        Ok(())
    }

    async fn sync_to_block(&mut self, latest_block: &GroupConsensusBlock, remote: ObjectId) {
        let fetch_height_immediate = self.store.header_height() + 3;

        if latest_block.height() <= fetch_height_immediate {
            // little blocks, get them from remote immediately.
            if self
                .sync_to_block_by_get_prev(latest_block.clone(), remote)
                .await
                .is_err()
            {
                self.network_sender
                    .post_message(
                        HotstuffMessage::SyncRequest(
                            SyncBound::Height(self.store.header_height() + 1),
                            SyncBound::Height(latest_block.height() - 1),
                        ),
                        self.rpath.clone(),
                        &remote,
                    )
                    .await;

                self.synchronizer.push_outorder_block(
                    latest_block.clone(),
                    fetch_height_immediate,
                    remote,
                );
            }
        } else {
            // large blocks, notify remote to push.
            log::debug!(
                "[hotstuff] local: {:?}, will sync blocks from height({}) to height({}). block.round={}, remote: {}",
                self,
                self.store.max_height(),
                latest_block.height(),
                latest_block.round(),
                remote
            );

            if self.store.max_height() + 1 <= fetch_height_immediate - 1 {
                self.network_sender
                    .post_message(
                        HotstuffMessage::SyncRequest(
                            SyncBound::Height(self.store.max_height() + 1),
                            SyncBound::Height(fetch_height_immediate - 1),
                        ),
                        self.rpath.clone(),
                        &remote,
                    )
                    .await;
            }

            self.synchronizer.push_outorder_block(
                latest_block.clone(),
                fetch_height_immediate,
                remote,
            );
        }
    }

    async fn sync_to_block_by_get_prev(
        &mut self,
        latest_block: GroupConsensusBlock,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        // 1. fetch prev blocks in range (header_height, latest_block.height)
        let header_height = self.store.header_height();
        let max_height = latest_block.height();
        if max_height <= header_height {
            return Ok(());
        }

        let mut blocks = Vec::with_capacity((max_height - header_height) as usize);
        let mut fetching_block_id = latest_block.prev_block_id().cloned();

        blocks.push(latest_block);

        for i in 0..(max_height - header_height - 1) {
            let expected_height = max_height - i - 1;
            match fetching_block_id.as_ref() {
                Some(block_id) => {
                    if self.store.find_block_in_cache(block_id).is_ok() {
                        break;
                    }

                    let block = self
                        .non_driver
                        .get_block(block_id, Some(&remote))
                        .await
                        .map_err(|err| {
                            log::error!(
                                "[hotstuff] local: {:?}, sync block({}) at height({}) from {} failed, err: {:?}.",
                                self,
                                block_id,
                                expected_height,
                                remote,
                                err
                            );
                            err
                        })?;

                    if block.height() != expected_height {
                        log::error!("[hotstuff] local: {:?}, sync block({}) at height({}) from {} failed, block.height is {}.", self, block_id, expected_height, remote, block.height());
                        return Err(BuckyError::new(
                            BuckyErrorCode::Unmatch,
                            "unexpected block height",
                        ));
                    }

                    log::debug!(
                        "[hotstuff] local: {:?}, sync block({}) at height({}) from {}, fetch success.",
                        self,
                        block_id,
                        expected_height,
                        remote
                    );

                    fetching_block_id = block.prev_block_id().cloned();
                    blocks.push(block);
                }
                None => {
                    log::error!("[hotstuff] local: {:?}, sync block at height({}) from {} failed, block id is None.", self, expected_height, remote);
                    return Err(BuckyError::new(
                        BuckyErrorCode::Failed,
                        "unexpected block id",
                    ));
                }
            }
        }

        // 2. handle blocks in order
        for block in blocks.into_iter().rev() {
            let block_id = block.block_id().clone();
            let block_height = block.height();

            log::debug!(
                "[hotstuff] local: {:?}, sync block({}) at height({}) from {}, will handle it.",
                self,
                block_id,
                block_height,
                remote
            );

            if let Err(err) = self.tx_message_inner.send((block, remote)).await {
                log::warn!(
                    "[hotstuff] local: {:?}, sync block({}) at height({}) from {}, send got block failed, err: {:?}.",
                    self,
                    block_id,
                    block_height,
                    remote,
                    err
                );
            }
        }

        Ok(())
    }

    async fn handle_query_state(&self, sub_path: String, remote: ObjectId) -> BuckyResult<()> {
        let result = self.store.get_by_path(sub_path.as_str()).await;
        self.network_sender
            .post_message(
                HotstuffMessage::VerifiableState(sub_path, result),
                self.rpath.clone(),
                &remote,
            )
            .await;

        Ok(())
    }

    async fn check_group_is_latest(&self, group_shell_id: &ObjectId) -> BuckyResult<bool> {
        let (_, latest_shell_id) = self.shell_mgr.group();
        if &latest_shell_id == group_shell_id {
            Ok(true)
        } else {
            let latest_group = self
                .shell_mgr
                .get_group(self.rpath.group_id(), None, None)
                .await?;
            let group_shell = latest_group.to_shell();
            let latest_shell_id = group_shell.shell_id();
            Ok(&latest_shell_id == group_shell_id)
        }
    }

    async fn make_sure_result_state(
        &self,
        result_state_id: &ObjectId,
        remotes: &[&ObjectId],
    ) -> BuckyResult<()> {
        // TODO: 需要一套通用的同步ObjectMap树的实现，这里缺少对于异常的处理
        let obj_map_processor = self.store.get_object_map_processor();
        let local_trace_log = format!("{:?}", self);

        #[async_recursion::async_recursion]
        async fn make_sure_sub_tree(
            root_id: &ObjectId,
            non_driver: crate::network::NONDriverHelper,
            remote: &ObjectId,
            obj_map_processor: &dyn GroupObjectMapProcessor,
            local_trace_log: &str,
        ) -> BuckyResult<()> {
            if root_id.is_data() {
                return Ok(());
            }

            if non_driver.get_object(&root_id, None).await.is_ok() {
                // TODO: 可能有下级分支子树因为异常不齐全
                log::debug!(
                    "[hotstuff] {} make_sure_result_state {} already exist.",
                    local_trace_log,
                    root_id
                );
                return Ok(());
            }
            let obj = non_driver
                .get_object(root_id, Some(remote))
                .await
                .map_err(|err| {
                    log::warn!(
                        "[hotstuff] {} get branch {} failed {:?}",
                        local_trace_log,
                        root_id,
                        err
                    );
                    err
                })?;
            match obj.object.as_ref() {
                Some(obj) if obj.obj_type_code() == ObjectTypeCode::ObjectMap => {
                    let single_op_env = obj_map_processor.create_single_op_env().await.map_err(|err| {
                        log::warn!("[hotstuff] {} make_sure_result_state {} create_single_op_env failed {:?}.", local_trace_log, root_id, err);
                        err
                    })?;
                    single_op_env.load(root_id).await.map_err(|err| {
                        log::warn!(
                            "[hotstuff] {} make_sure_result_state {} load failed {:?}.",
                            local_trace_log,
                            root_id,
                            err
                        );
                        err
                    })?;
                    loop {
                        let branchs = single_op_env.next(16).await?;
                        for branch in branchs.list.iter() {
                            let branch_id = match branch {
                                cyfs_base::ObjectMapContentItem::DiffMap(diff_map) => {
                                    match diff_map.1.altered.as_ref() {
                                        Some(branch_id) => branch_id,
                                        None => continue,
                                    }
                                }
                                cyfs_base::ObjectMapContentItem::Map(map) => &map.1,
                                cyfs_base::ObjectMapContentItem::DiffSet(diff_set) => {
                                    match diff_set.altered.as_ref() {
                                        Some(branch_id) => branch_id,
                                        None => continue,
                                    }
                                }
                                cyfs_base::ObjectMapContentItem::Set(set) => set,
                            };
                            make_sure_sub_tree(
                                branch_id,
                                non_driver.clone(),
                                remote,
                                obj_map_processor,
                                local_trace_log,
                            )
                            .await?;
                        }

                        if branchs.list.len() < 16 {
                            return Ok(());
                        }
                    }
                }
                _ => return Ok(()),
            }
        }

        let mut result = Ok(());
        for remote in remotes {
            result = make_sure_sub_tree(
                result_state_id,
                self.non_driver.clone(),
                remote,
                obj_map_processor,
                local_trace_log.as_str(),
            )
            .await;
            if result.is_ok() {
                return result;
            }
        }
        result
    }

    async fn recover(&mut self) {
        // Upon booting, generate the very first block (if we are the leader).
        // Also, schedule a timer in case we don't hear from the leader.
        let max_round_block = self.store.block_with_max_round();
        let group_shell_id = max_round_block.as_ref().map(|block| block.group_shell_id());
        let last_group = self
            .shell_mgr
            .get_group(self.rpath.group_id(), group_shell_id, None)
            .await;
        let latest_group = match group_shell_id.as_ref() {
            Some(_) => {
                self.shell_mgr
                    .get_group(self.rpath.group_id(), None, None)
                    .await
            }
            None => last_group.clone(),
        };

        let duration = latest_group
            .as_ref()
            .map_or(HOTSTUFF_TIMEOUT_DEFAULT, |_g| {
                GROUP_DEFAULT_CONSENSUS_INTERVAL
            });
        self.timer.reset(duration);

        if let Ok(leader) = self.committee.get_leader(None, self.round, None).await {
            if leader == self.local_device_id {
                match max_round_block {
                    Some(max_round_block)
                        if max_round_block.owner() == &self.local_id
                            && max_round_block.round() == self.round
                            && latest_group.is_ok()
                            && last_group.is_ok()
                            && last_group
                                .as_ref()
                                .unwrap()
                                .is_same_ood_list(latest_group.as_ref().unwrap()) =>
                    {
                        // discard the generated block when the ood-list is changed
                        self.broadcast(
                            HotstuffMessage::Block(max_round_block),
                            &latest_group.unwrap(),
                        );
                    }
                    _ => {
                        if let Err(err) = self.generate_block(self.with_tc()).await {
                            log::warn!("[hotstuff] {:?} recover generate new block failed, will wait it timeout for new block, err: {:?}.", self, err);
                        }
                    }
                }
            }
        }
    }

    fn proposal_waiter(waiter: Option<(Receiver<()>, u64)>) -> impl futures::Future<Output = u64> {
        async move {
            match waiter.as_ref() {
                Some((waiter, wait_round)) => {
                    if let Err(err) = waiter.recv().await {
                        log::warn!("[hotstuff] wait proposal failed, will try fetch proposals to generate new block, err: {:?}",  err);
                    }
                    *wait_round
                }
                None => std::future::pending::<u64>().await,
            }
        }
    }

    async fn run(&mut self) -> ! {
        log::info!("[hotstuff] {:?} start, will recover.", self);

        self.recover().await;

        log::info!("[hotstuff] {:?} start, recovered.", self);

        // This is the main loop: it processes incoming blocks and votes,
        // and receive timeout notifications from our Timeout Manager.
        loop {
            let result = futures::select! {
                message = self.rx_message.recv().fuse() => match message {
                    Ok((HotstuffMessage::Block(block), remote)) => {
                        if remote == self.local_device_id {
                            self.process_block(&block, remote, &HashMap::new()).await
                        } else {
                            self.handle_block(&block, remote).await
                        }
                    },
                    Ok((HotstuffMessage::BlockVote(vote), remote)) => self.handle_vote(&vote, None, remote).await,
                    Ok((HotstuffMessage::TimeoutVote(timeout), remote)) => self.handle_timeout(&timeout, remote).await,
                    Ok((HotstuffMessage::Timeout(tc), remote)) => self.handle_tc(&tc, remote).await,
                    Ok((HotstuffMessage::SyncRequest(min_bound, max_bound), remote)) => self.synchronizer.process_sync_request(min_bound, max_bound, remote, &self.store).await,
                    Ok((HotstuffMessage::LastStateRequest, _)) => panic!("should process by StatePusher"),
                    Ok((HotstuffMessage::StateChangeNotify(_, _), _)) => panic!("should process by DecStateSynchronizer"),
                    Ok((HotstuffMessage::ProposalResult(_, _), _)) => panic!("should process by DecStateSynchronizer"),
                    Ok((HotstuffMessage::QueryState(sub_path), remote)) => self.handle_query_state(sub_path, remote).await,
                    Ok((HotstuffMessage::VerifiableState(_, _), _)) => panic!("should process by DecStateRequestor"),
                    Err(e) => {
                        log::warn!("[hotstuff] rx_message closed, err: {:?}.", e);
                        Ok(())
                    },
                },
                message = self.rx_message_inner.recv().fuse() => match message {
                    Ok((block, remote)) => {
                        log::debug!(
                            "[hotstuff] local: {:?}, receive block({}) from ({}) from rx_message_inner, height: {}, round: {}.",
                            self,
                            block.block_id(),
                            remote,
                            block.height(),
                            block.round()
                        );

                        if remote == self.local_device_id {
                            self.process_block(&block, remote, &HashMap::new()).await
                        } else {
                            self.handle_block(&block, remote).await
                        }
                    },
                    Err(e) => {
                        log::warn!("[hotstuff] rx_message_inner closed, err: {:?}.", e);
                        Ok(())
                    },
                },
                block = self.rx_block_gen.recv().fuse() => match block {
                    Ok((block, proposals)) => self.process_block(&block, self.local_device_id, &proposals).await,
                    Err(e) => {
                        log::warn!("[hotstuff] rx_block_gen closed, err: {:?}.", e);
                        Ok(())
                    }
                },
                () = self.timer.wait_next().fuse() => self.local_timeout_round().await,
                wait_round = Self::proposal_waiter(self.rx_proposal_waiter.clone()).fuse() => {
                    self.rx_proposal_waiter = None;
                    if wait_round == self.round {
                        // timeout
                        self.handle_proposal_waiting().await
                    } else {
                        Ok(())
                    }
                }
            };

            log::debug!(
                "[hotstuff] local: {:?}, new message response. result: {:?}",
                self,
                result
            );
        }
    }

    fn debug_identify(&self) -> String {
        format!("{:?}-{:?}-{}", self.rpath, self.local_device_id, self.round)
    }
}
