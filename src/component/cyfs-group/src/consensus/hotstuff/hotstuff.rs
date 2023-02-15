use std::{collections::HashMap, sync::Arc, time::{SystemTime, Duration}};

use async_std::channel::{Receiver, Sender};
use cyfs_base::{
    bucky_time_to_system_time, BuckyError, BuckyErrorCode, BuckyResult, Group, NamedObject,
    ObjectDesc, ObjectId, ObjectLink, OwnerObjectDesc, RawConvertTo, RawDecode, RawEncode,
    RsaCPUObjectSigner, SignatureSource, Signer,
};
use cyfs_chunk_lib::ChunkMeta;
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, GroupConsensusBlockProposal, GroupProposal,
    GroupProposalObject, GroupRPath, HotstuffBlockQC, HotstuffTimeout,
};
use cyfs_lib::NONObjectInfo;
use futures::FutureExt;

use crate::{
    consensus::{synchronizer::Synchronizer, proposal}, dec_state::StatePusher, helper::Timer, Committee,
    ExecuteResult, GroupStorage, HotstuffBlockQCVote, HotstuffMessage, HotstuffTimeoutVote,
    PendingProposalConsumer, RPathDelegate, SyncBound, VoteMgr, VoteThresholded, CHANNEL_CAPACITY,
    HOTSTUFF_TIMEOUT_DEFAULT, TIME_PRECISION, PROPOSAL_MAX_TIMEOUT,
};

/**
 * TODO: generate empty block when the 'Node' is synchronizing
*/

pub(crate) struct Hotstuff {
    rpath: GroupRPath,
    local_device_id: ObjectId,
    tx_message: Sender<(HotstuffMessage, ObjectId)>,
    state_pusher: StatePusher,
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
        proposal_consumer: PendingProposalConsumer,
        delegate: Arc<Box<dyn RPathDelegate>>,
        rpath: GroupRPath,
    ) -> Self {
        let (tx_message, rx_message) = async_std::channel::bounded(CHANNEL_CAPACITY);

        let state_pusher = StatePusher::new(
            local_id,
            network_sender.clone(),
            rpath.clone(),
            non_driver.clone(),
        );

        let tx_message_runner = tx_message.clone();
        let state_pusher_runner = state_pusher.clone();

        {
            let rpath2 = rpath.clone();
            async_std::task::spawn(async move {
                HotstuffRunner::new(
                    local_id,
                    local_device_id,
                    committee,
                    store,
                    signer,
                    network_sender,
                    non_driver,
                    tx_message_runner,
                    rx_message,
                    proposal_consumer,
                    state_pusher_runner,
                    delegate,
                    rpath2,
                )
                .run()
                .await
            });            
        }

        Self {
            local_device_id,
            tx_message,
            state_pusher,
            rpath
        }
    }

    pub async fn on_block(&self, block: cyfs_core::GroupConsensusBlock, remote: ObjectId) {
        log::debug!("[hotstuff] local: {:?}, on_block: {:?}/{:?}/{:?}, prev: {:?}/{:?}, owner: {:?}, remote: {:?},",
            self,
            block.block_id(), block.round(), block.height(),
            block.prev_block_id(), block.qc().as_ref().map_or(0, |qc| qc.round),
            block.owner(), remote);

        self.tx_message
            .send((HotstuffMessage::Block(block), remote))
            .await;
    }

    pub async fn on_block_vote(&self, vote: HotstuffBlockQCVote, remote: ObjectId) {
        log::debug!("[hotstuff] local: {:?}, on_block_vote: {:?}/{:?}, prev: {:?}, voter: {:?}, remote: {:?},",
            self,
            vote.block_id, vote.round,
            vote.prev_block_id,
            vote.voter, remote);

        self.tx_message
            .send((HotstuffMessage::BlockVote(vote), remote))
            .await;
    }

    pub async fn on_timeout_vote(&self, vote: HotstuffTimeoutVote, remote: ObjectId) {
        log::debug!(
            "[hotstuff] local: {:?}, on_timeout_vote: {:?}, qc: {:?}, voter: {:?}, remote: {:?},",
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

        self.tx_message
            .send((HotstuffMessage::TimeoutVote(vote), remote))
            .await;
    }

    pub async fn on_timeout(&self, tc: HotstuffTimeout, remote: ObjectId) {
        log::debug!(
            "[hotstuff] local: {:?}, on_timeout: {:?}, voter: {:?}, remote: {:?},",
            self,
            tc.round,
            tc.votes
                .iter()
                .map(|vote| format!("{:?}/{:?}", vote.high_qc_round, vote.voter,))
                .collect::<Vec<String>>(),
            remote
        );

        self.tx_message
            .send((HotstuffMessage::Timeout(tc), remote))
            .await;
    }

    pub async fn on_sync_request(
        &self,
        min_bound: SyncBound,
        max_bound: SyncBound,
        remote: ObjectId,
    ) {
        log::debug!(
            "[hotstuff] local: {:?}, on_sync_request: min: {:?}, max: {:?}, remote: {:?},",
            self,
            min_bound,
            max_bound,
            remote
        );

        self.tx_message
            .send((HotstuffMessage::SyncRequest(min_bound, max_bound), remote))
            .await;
    }

    pub async fn request_last_state(&self, remote: ObjectId) {
        log::debug!(
            "[hotstuff] local: {:?}, on_sync_request: remote: {:?},",
            self,
            remote
        );

        self.state_pusher.request_last_state(remote).await;
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
    timer: Timer, // 定时器
    vote_mgr: VoteMgr,
    network_sender: crate::network::Sender,
    non_driver: crate::network::NONDriverHelper,
    tx_message: Sender<(HotstuffMessage, ObjectId)>,
    rx_message: Receiver<(HotstuffMessage, ObjectId)>,
    tx_block_gen: Sender<(GroupConsensusBlock, HashMap<ObjectId, GroupProposal>)>,
    rx_block_gen: Receiver<(GroupConsensusBlock, HashMap<ObjectId, GroupProposal>)>,
    proposal_consumer: PendingProposalConsumer,
    delegate: Arc<Box<dyn RPathDelegate>>,
    synchronizer: Synchronizer,
    rpath: GroupRPath,
    rx_proposal_waiter: Option<(Receiver<()>, u64)>,
    state_pusher: StatePusher,
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
        tx_message: Sender<(HotstuffMessage, ObjectId)>,
        rx_message: Receiver<(HotstuffMessage, ObjectId)>,
        proposal_consumer: PendingProposalConsumer,
        state_pusher: StatePusher,
        delegate: Arc<Box<dyn RPathDelegate>>,
        rpath: GroupRPath,
    ) -> Self {
        let max_round_block = store.block_with_max_round();

        let round = store
            .last_vote_round()
            .max(max_round_block.as_ref().map_or(1, |block| block.round()));
        let high_qc = max_round_block.map_or(None, |block| block.qc().clone());

        let vote_mgr = VoteMgr::new(committee.clone(), round);
        let init_timer_interval = store.group().consensus_interval();
        let max_height = store.header_height() + 2;

        let synchronizer = Synchronizer::new(
            network_sender.clone(),
            rpath.clone(),
            max_height,
            round,
            tx_message.clone(),
        );

        let (tx_block_gen, rx_block_gen) = async_std::channel::bounded(1);

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
            delegate,
            synchronizer,
            non_driver,
            rpath,
            proposal_consumer,
            rx_proposal_waiter: None,
            tc: None,
            state_pusher,
            tx_block_gen,
            rx_block_gen,
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

        let latest_group = self.committee.get_group(None).await?;
        if !latest_group.contain_ood(&remote) {
            log::warn!(
                "[hotstuff] local: {:?}, receive block({}) from unknown({})",
                self,
                block.block_id(),
                remote
            );
            return Ok(());
        }

        /**
         * 1. 验证block投票签名
         * 2. 验证出块节点
         * 3. 同步块
         * 4. 验证各个proposal执行结果
         */
        Self::check_block_result_state(block)?;

        log::debug!("[hotstuff] local: {:?}, handle_block-step2: {:?}",
            self,
            block.block_id());

        {
            // check leader
            let leader_owner = self.get_leader_owner(Some(block.group_chunk_id()), block.round()).await?;

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

        let (prev_block, proposals) = match self.check_block_linked(&block, remote).await {
            Ok(link) => link,
            Err(err) => return err
        };

        log::debug!("[hotstuff] local: {:?}, handle_block-step3: {:?}",
            self,
            block.block_id());

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

        log::debug!("[hotstuff] local: {:?}, handle_block-step4: {:?}",
            self,
            block.block_id());

        self.check_block_proposal_result_state_by_app(block, &proposals, &prev_block)
            .await?;

        self.synchronizer.pop_link_from(block);

        self.process_qc(block.qc()).await;

        if let Some(tc) = block.tc() {
            self.advance_round(tc.round).await;
        }

        self.process_block(block, remote, &proposals).await
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

    async fn check_block_proposal_result_state_by_app(
        &self,
        block: &GroupConsensusBlock,
        proposals: &HashMap<ObjectId, GroupProposal>,
        prev_block: &Option<GroupConsensusBlock>,
    ) -> BuckyResult<()> {
        let mut prev_state_id = prev_block
            .as_ref()
            .map_or(None, |block| block.result_state_id().clone());

        for proposal_exe_info in block.proposals() {
            // 去重
            if let Some(prev_block_id) = block.prev_block_id() {
                if self.store
                    .is_proposal_finished(&proposal_exe_info.proposal, prev_block_id)
                    .await.map_err(|err| {
                        log::warn!("[hotstuff] local: {:?}, check proposal {:?} in block {:?} with prev-block {:?} duplicate failed, {:?}."
                            , self, proposal_exe_info.proposal, block.block_id(), prev_block_id, err);
                        err
                    })? {
                        log::warn!("[hotstuff] local: {:?}, proposal {:?} in block {:?} with prev-block {:?} has finished before."
                            , self, proposal_exe_info.proposal, block.block_id(), prev_block_id);
                        
                        return Err(BuckyError::new(BuckyErrorCode::ErrorState, "duplicate proposal"))
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
                },
                None => None,
            };

            let exe_result = ExecuteResult {
                result_state_id: proposal_exe_info.result_state,
                receipt,
                context: proposal_exe_info.context.clone(),
            };

            if self
                .delegate
                .on_verify(proposal, prev_state_id, &exe_result)
                .await.map_err(|err| {
                    log::warn!("[hotstuff] local: {:?}, proposal {:?} in block {:?} verify by app failed {:?}."
                        , self, proposal_exe_info.proposal, block.block_id(), err);
                    err
                })?
            {
                prev_state_id = proposal_exe_info.result_state;
            } else {
                log::warn!(
                    "[hotstuff] local: {:?}, block verify failed by app, proposal: {}, prev_state: {:?}, expect-result: {:?}",
                    self,
                    proposal_exe_info.proposal,
                    prev_state_id,
                    proposal_exe_info.result_state
                );

                return Err(BuckyError::new(BuckyErrorCode::Reject, "verify failed"));
            }
        }

        assert_eq!(
            &prev_state_id,
            block.result_state_id(),
            "the result state is unmatched"
        );

        Ok(())
    }

    async fn get_leader_owner(&self, group_chunk_id: Option<&ObjectId>, round: u64) -> BuckyResult<ObjectId> {
        let leader = self
            .committee
            .get_leader(group_chunk_id, round)
            .await.map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, get leader from group {:?} with round {} failed, {:?}.",
                    self,
                    group_chunk_id, round,
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
                log::warn!("[hotstuff] local: {:?}, a owner must be set to the device {}",
                    self,
                    leader
                );
                Err(BuckyError::new(BuckyErrorCode::InvalidTarget, "no owner for device"))
            }
        }
    }

    async fn check_block_linked(&mut self, block: &GroupConsensusBlock, remote: ObjectId) -> Result<(Option<GroupConsensusBlock>, HashMap<ObjectId, GroupProposal>), BuckyResult<()>> {
        match self.store.block_linked(block).await
            .map_err(|err| Err(err))? {

            crate::storage::BlockLinkState::Expired => {
                log::warn!(
                    "[hotstuff] local: {:?}, receive block expired.",
                    self
                );
                Err(Err(BuckyError::new(BuckyErrorCode::Ignored, "expired")))
            }
            crate::storage::BlockLinkState::DuplicateProposal => {
                log::warn!(
                    "[hotstuff] local: {:?}, receive block with duplicate proposal.",
                    self
                );
                Err (Err(BuckyError::new(
                    BuckyErrorCode::AlreadyExists,
                    "duplicate proposal",
                )))
            }
            crate::storage::BlockLinkState::Duplicate => {
                log::warn!(
                    "[hotstuff] local: {:?}, receive duplicate block.",
                    self
                );
                Err( Err(BuckyError::new(
                    BuckyErrorCode::AlreadyExists,
                    "duplicate block",
                )))
            }
            crate::storage::BlockLinkState::Link(prev_block, proposals) => {
                log::debug!(
                    "[hotstuff] local: {:?}, receive in-order block, height: {}.",
                    self,
                    block.height()
                );

                // 顺序连接状态
                Self::check_empty_block_result_state_with_prev(block, &prev_block).map_err(|err| Err(err))?;
                Ok((prev_block, proposals))
            }
            crate::storage::BlockLinkState::Pending => {
                log::warn!(
                    "[hotstuff] local: {:?}, receive out-order block, expect height: {}, get height: {}.",
                    self,
                    self.store.header_height() + 3,
                    block.height()
                );

                // 乱序，同步
                if block.height() <= self.store.header_height() + 3 {
                    self.fetch_block(block.prev_block_id().unwrap(), remote)
                        .await;
                }

                let max_round_block = self.store.block_with_max_round();
                self.synchronizer.push_outorder_block(
                    block.clone(),
                    max_round_block.map_or(1, |block| block.height() + 1),
                    remote,
                );

                Err(Ok(()))
            }
            crate::storage::BlockLinkState::InvalidBranch => {
                log::warn!(
                    "[hotstuff] local: {:?}, receive block in invalid branch.",
                    self
                );
                Err( Err(BuckyError::new(BuckyErrorCode::Conflict, "conflict branch")))
            }
        }
    }

    async fn process_block(
        &mut self,
        block: &GroupConsensusBlock,
        remote: ObjectId,
        proposals: &HashMap<ObjectId, GroupProposal>
    ) -> BuckyResult<()> {
        /**
         * 验证过的块执行这个函数
         */

        log::info!(
            "[hotstuff] local: {:?}, will push new block {:?}/{}/{} to storage",
            self, block.block_id(), block.height(), block.round()
        );

        let debug_identify = self.debug_identify();
        let new_header_block = self.store.push_block(block.clone()).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, push verified block {:?} to storage failed {:?}",
                debug_identify, block.block_id(), err
            );

            err
        })?;

        if let Some(header_block) = new_header_block.map(|b| b.0.clone()) {
            log::info!(
                "[hotstuff] local: {:?}, new header-block {:?} committed",
                self, header_block.block_id()
            );

            /**
             * 这里只清理已经提交的block包含的proposal
             * 已经执行过的待提交block包含的proposal在下次打包时候去重
             * */
            self.cleanup_proposal(&header_block).await;

            log::debug!(
                "[hotstuff] local: {:?}, process_block-step1 {:?}",
                self, block.block_id()
            );

            let (_, qc_block) = self
                .store
                .pre_commits()
                .iter()
                .next()
                .expect("the pre-commit block must exist.");

            self.notify_block_committed(header_block.clone(), qc_block).await;

            log::debug!(
                "[hotstuff] local: {:?}, process_block-step2 {:?}",
                self, block.block_id()
            );

            let leader = self.committee.get_leader(None, self.round).await.map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, get leader in round {} failed {:?}",
                    self, self.round, err
                );

                err
            });

            // notify by leader
            if let Ok(leader) = leader {
                if self.local_device_id == leader {
                    self.state_pusher
                        .notify_block_commit(header_block, qc_block.clone())
                        .await;
                }
            }

            log::debug!(
                "[hotstuff] local: {:?}, process_block-step3 {:?}",
                self, block.block_id()
            );
        }

        match self.vote_mgr.add_voting_block(block).await {
            VoteThresholded::QC(qc) => {
                log::debug!(
                    "[hotstuff] local: {:?}, the qc of block {:?} has received before",
                    self, block.block_id()
                );
                return self.process_block_qc(qc, block, remote).await;
            },
            VoteThresholded::TC(tc, max_high_qc_block) => {
                log::debug!(
                    "[hotstuff] local: {:?}, the timeout-qc of block {:?} has received before",
                    self, block.block_id()
                );

                return self
                    .process_timeout_qc(tc, max_high_qc_block.as_ref())
                    .await
            }
            VoteThresholded::None => {}
        }

        log::debug!(
            "[hotstuff] local: {:?}, process_block-step4 {:?}",
            self, block.block_id()
        );

        if block.round() != self.round {
            log::debug!(
                "[hotstuff] local: {:?}, not my round {}, expect {}",
                self, block.round(), self.round
            );
            // 不是我的投票round
            return Ok(());
        }

        if let Some(vote) = self.make_vote(block, proposals).await {
            log::info!("[hotstuff] local: {:?}, vote to block {}, round: {}",
                self, block.block_id(), block.round());

            let next_leader = self.committee.get_leader(None, self.round + 1).await.map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, get next leader in round {} failed {:?}",
                    self, self.round + 1, err
                );

                err
            })?;

            if self.local_device_id == next_leader {
                self.handle_vote(&vote, Some(block), self.local_device_id).await?;
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

    async fn notify_block_committed(&self, new_header: GroupConsensusBlock, qc_block: &GroupConsensusBlock) -> BuckyResult<()> {
        let mut pre_state_id = match new_header.prev_block_id() {
            Some(block_id) => self
                .non_driver
                .get_block(block_id, None)
                .await.map_err(|err| {
                    log::warn!(
                        "[hotstuff] local: {:?}, get prev-block {:?} before commit-notify failed {:?}",
                        self, block_id, err
                    );
                    err
                })?
                .result_state_id()
                .clone(),
            None => None,
        };

        for proposal in new_header.proposals() {
            let proposal_obj = self
                .non_driver
                .get_proposal(&proposal.proposal, None)
                .await.map_err(|err| {
                    log::warn!(
                        "[hotstuff] local: {:?}, get proposal {:?} in header-block {:?} before commit-notify failed {:?}",
                        self, proposal.proposal, new_header.block_id(), err
                    );

                    err
                })?;
            let receipt = match proposal.receipt.as_ref() {
                Some(receipt) => {
                    let (receipt, remain) = NONObjectInfo::raw_decode(receipt.as_slice()).map_err(|err| {
                        log::warn!(
                            "[hotstuff] local: {:?}, decode receipt of proposal {:?} in header-block {:?} before commit-notify failed {:?}",
                            self, proposal.proposal, new_header.block_id(), err
                        );
                        err
                    })?;
                    assert_eq!(remain.len(), 0);
                    Some(receipt)
                }
                None => None,
            };

            self.delegate
                .on_commited(
                    &proposal_obj,
                    pre_state_id,
                    &ExecuteResult {
                        result_state_id: proposal.result_state.clone(),
                        receipt,
                        context: proposal.context.clone(),
                    },
                    &new_header,
                )
                .await;
            
            pre_state_id = proposal.result_state.clone();
        }

        Ok(())
    }

    async fn process_qc(&mut self, qc: &Option<HotstuffBlockQC>) {
        let qc_round = qc.as_ref().map_or(0, |qc| qc.round);

        log::debug!("[hotstuff] local: {:?}, process_qc round {}",
            self, qc_round);

        self.advance_round(qc_round).await;
        self.update_high_qc(qc);
    }

    async fn advance_round(&mut self, round: u64) {
        if round < self.round {
            log::debug!("[hotstuff] local: {:?}, round {} timeout expect {}",
                self, round, self.round);
            return;
        }

        match  self.committee.get_group(None).await {
            Ok(group) => {
                log::info!("[hotstuff] local: {:?}, update round from {} to {}",
                    self, self.round, round + 1);

                self.timer.reset(group.consensus_interval());
                self.round = round + 1;
                self.vote_mgr.cleanup(self.round);
                self.tc = None;
            }
            Err(err) => {
                log::warn!("[hotstuff] local: {:?}, get group before update round from {} to {} failed {:?}",
                    self, self.round, round + 1, err);
            }
        }
    }

    fn update_high_qc(&mut self, qc: &Option<HotstuffBlockQC>) {
        let to_high_round = qc.as_ref().map_or(0, |qc| qc.round);
        let cur_high_round = self.high_qc.as_ref().map_or(0, |qc| qc.round);
        if to_high_round > cur_high_round {
            self.high_qc = qc.clone();

            log::info!("[hotstuff] local: {:?}, update high-qc from {} to {}",
                self, cur_high_round, to_high_round);
        }
    }

    async fn cleanup_proposal(&mut self, commited_block: &GroupConsensusBlock) -> BuckyResult<()> {
        let proposals = commited_block
            .proposals()
            .iter()
            .map(|proposal| proposal.proposal)
            .collect::<Vec<_>>();

        log::debug!("[hotstuff] local: {:?}, remove proposals: {:?}",
            self, proposals.len());

        self.proposal_consumer.remove_proposals(proposals).await
    }

    async fn notify_proposal_err(&self, proposal: &GroupProposal, err: BuckyError) {
        log::debug!("[hotstuff] local: {:?}, proposal {} failed {:?}",
            self, proposal.desc().object_id(), err);

        self.state_pusher
            .notify_proposal_err(proposal.clone(), err)
            .await;
    }

    async fn make_vote(&mut self, block: &GroupConsensusBlock, mut proposals: &HashMap<ObjectId, GroupProposal>) -> Option<HotstuffBlockQCVote> {
        if block.round() <= self.store.last_vote_round() {
            log::debug!("[hotstuff] local: {:?}, make vote ignore for timeouted block {}/{}, last vote roud: {}",
                self, block.block_id(), block.round(), self.store.last_vote_round());

            return None;
        }

        // 时间和本地误差太大，不签名，打包的proposal时间和block时间差距太大，也不签名
        let mut proposal_temp: HashMap<ObjectId, GroupProposal> = HashMap::new();
        if proposals.len() == 0 && block.proposals().len() > 0 {
            match self.non_driver.load_all_proposals_for_block(block, &mut proposal_temp).await {
                Ok(_) => proposals = &proposal_temp,
                Err(_) => return None
            }
        } else {
            assert_eq!(proposals.len(), block.proposals().len());
        }
        if !Self::check_timestamp_precision(block, proposals) {
            return None;
        }

        // round只能逐个递增
        let qc_round = block.qc().as_ref().map_or(0, |qc| qc.round);
        let is_valid_round = if block.round() == qc_round + 1 {
            true
        } else if let Some(tc) = block.tc() {
            block.round() == tc.round + 1
                && qc_round
                    >= tc.votes.iter().map(|v| v.high_qc_round).max().unwrap()
        } else {
            false
        };

        if !is_valid_round {
            log::warn!("[hotstuff] local: {:?}, make vote to block {} ignore for invalid round {}, qc-round {}, tc-round {}",
                self,
                block.block_id(),
                block.round(), qc_round,
                block.tc().as_ref().map_or(0, |tc| tc.votes.iter().map(|v| v.high_qc_round).max().unwrap()));

            return None;
        }

        match self.check_group_is_latest(block.group_chunk_id()).await {
            Ok(is_latest) if is_latest => {}
            _ => {
                log::warn!("[hotstuff] local: {:?}, make vote to block {} ignore for the group is not latest",
                    self,
                    block.block_id());

                return None;
            }
        }

        log::debug!("[hotstuff] local: {:?}, make-vote before sign {}, round: {}",
            self, block.block_id(), block.round());

        let vote = match HotstuffBlockQCVote::new(block, self.local_device_id, &self.signer).await {
            Ok(vote) => {
                log::debug!("[hotstuff] local: {:?}, make-vote after sign {}, round: {}",
                    self, block.block_id(), block.round());
    
                vote
            },
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

        if self.store.set_last_vote_round(block.round()).await.is_err() {
            return None;
        }

        Some(vote)
    }

    fn check_timestamp_precision(block: &GroupConsensusBlock, proposals: &HashMap<ObjectId, GroupProposal>) -> bool {
        let now = SystemTime::now();
        let block_timestamp = bucky_time_to_system_time(block.named_object().desc().create_time());
        if Self::calc_time_delta(now, block_timestamp) > TIME_PRECISION {
            false
        } else {
            for proposal in block.proposals() {
                let proposal = proposals.get(&proposal.proposal).expect("should load all proposals");
                let proposal_timestamp = bucky_time_to_system_time(proposal.desc().create_time());
                if Self::calc_time_delta(block_timestamp, proposal_timestamp) > TIME_PRECISION {
                    return false
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
                vote.block_id, vote.round, vote.prev_block_id,
                self.round
            );
            return Ok(());
        }

        self.committee.verify_vote(vote).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, verify vote({}/{}/{:?}) failed {:?}",
                self,
                vote.block_id, vote.round, vote.prev_block_id,
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
        let qc = self.vote_mgr.add_vote(vote.clone(), prev_block).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, add vote({}/{}/{:?}) prev-block: {} failed {:?}",
                self,
                vote.block_id, vote.round, vote.prev_block_id,
                if is_prev_none {"None"} else {"Some"},
                err
            );
            err
        })?;

        if let Some((qc, block)) = qc {
            log::info!(
                "[hotstuff] local: {:?}, vote({}/{}/{:?}) prev-block: {} qc",
                self,
                vote.block_id, vote.round, vote.prev_block_id,
                if is_prev_none {"None"} else {"Some"}
            );

            self.process_block_qc(qc, &block, remote).await?;
        } else if vote.round > self.round && is_prev_none {
            self.fetch_block(&vote.block_id, remote).await?;
        }
        Ok(())
    }

    async fn process_block_qc(
        &mut self,
        qc: HotstuffBlockQC,
        prev_block: &GroupConsensusBlock,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        let qc_block_id = qc.block_id;
        let qc_round = qc.round;
        let qc_prev_block_id = qc.prev_block_id;

        self.process_qc(&Some(qc)).await;

        let new_leader = self.committee.get_leader(None, self.round).await.map_err(|err| {
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
            self.generate_block(None).await;
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

                    self.vote_mgr.add_waiting_timeout(timeout.clone());
                    self.fetch_block(&qc.block_id, remote).await;
                    return Ok(());
                }
            },
            None => None,
        };

        self.committee
            .verify_timeout(timeout, block.as_ref())
            .await.map_err(|err| {
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
            .add_timeout(timeout.clone(), block.as_ref())
            .await.map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, handle_timeout: {:?}, check tc failed {:?}",
                    self,
                    timeout.round,
                    err
                );
                err
            })?;

        if let Some((tc, max_high_qc_block)) = tc {
            self.process_timeout_qc(tc, max_high_qc_block.as_ref())
                .await?;
        }
        Ok(())
    }

    async fn process_timeout_qc(
        &mut self,
        tc: HotstuffTimeout,
        max_high_qc_block: Option<&GroupConsensusBlock>,
    ) -> BuckyResult<()> {
        log::debug!(
            "[hotstuff] local: {:?}, process_timeout_qc: {:?}, voter: {:?}, high-qc block: {:?},",
            self,
            tc.round,
            tc.votes
                .iter()
                .map(|vote| format!("{:?}/{:?}", vote.high_qc_round, vote.voter,))
                .collect::<Vec<String>>(),
            max_high_qc_block.as_ref().map(|qc| qc.prev_block_id())
        );

        self.advance_round(tc.round).await;
        self.tc = Some(tc.clone());

        let new_leader = self.committee.get_leader(None, self.round).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, process_timeout_qc: {:?}, get new leader failed {:?}",
                self,
                tc.round,
                err
            );

            err
        })?;
        if self.local_device_id == new_leader {
            self.generate_block(Some(tc)).await;
            Ok(())
        } else {
            let latest_group = self.committee.get_group(None).await.map_err(|err| {
                log::warn!(
                    "[hotstuff] local: {:?}, process_timeout_qc: {:?}, get group failed {:?}",
                    self,
                    tc.round,
                    err
                );
                err
            })?;

            self.broadcast(HotstuffMessage::Timeout(tc), &latest_group)
        }
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
            None => return Ok(())
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
                tc.round, max_high_qc.high_qc_round
            );

            return Ok(());
        }

        let block = if max_high_qc.high_qc_round == 0 {
            None
        } else {
            let block = match self
                .store
                .find_block_in_cache_by_round(max_high_qc.high_qc_round)
            {
                Ok(block) => block,
                Err(err) => {
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
                    return Ok(());
                }
            };
            Some(block)
        };

        self.committee.verify_tc(tc, block.as_ref()).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, handle_tc: {:?} verify tc failed {:?}",
                self,
                tc.round,
                err
            );
            err
        })?;

        self.advance_round(tc.round).await;
        self.tc = Some(tc.clone());

        let new_leader = self.committee.get_leader(None, self.round).await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, handle_tc: {:?} get new leader failed {:?}",
                self,
                tc.round,
                err
            );
            err
        })?;

        if self.local_device_id == new_leader {
            self.generate_block(Some(tc.clone())).await;
        }
        Ok(())
    }

    async fn local_timeout_round(&mut self) -> BuckyResult<()> {
        log::debug!(
            "[hotstuff] local: {:?}, local_timeout_round",
            self,
        );

        let latest_group = match self.committee.get_group(None).await {
            Ok(group) => {
                self.timer.reset(group.consensus_interval());
                group
            }
            Err(err) => {
                log::warn!(
                    "[hotstuff] local: {:?}, local_timeout_round get latest group failed {:?}",
                    self, err
                );

                self.timer.reset(HOTSTUFF_TIMEOUT_DEFAULT);
                return Err(err);
            }
        };

        let timeout = HotstuffTimeoutVote::new(
            self.high_qc.clone(),
            self.round,
            self.local_device_id,
            &self.signer,
        )
        .await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, local_timeout_round create new timeout-vote failed {:?}",
                self, err
            );
            err
        })?;

        self.store.set_last_vote_round(self.round).await?;

        self.broadcast(HotstuffMessage::TimeoutVote(timeout.clone()), &latest_group);
        self.tx_message.send((HotstuffMessage::TimeoutVote(timeout), self.local_device_id)).await;

        Ok(())
    }

    async fn generate_block(&mut self, tc: Option<HotstuffTimeout>) -> BuckyResult<()> {
        let now = SystemTime::now();

        log::debug!(
            "[hotstuff] local: {:?}, generate_block with qc {:?} and tc {:?}, now: {:?}",
            self,
            self.high_qc.as_ref().map(|qc| format!("{}/{}/{:?}", qc.block_id, qc.round, qc.votes.iter().map(|v| v.voter).collect::<Vec<_>>())),
            tc.as_ref().map(|tc| format!("{}/{:?}", tc.round, tc.votes.iter().map(|v| v.voter).collect::<Vec<_>>())),
            now
        );

        let mut proposals = self.proposal_consumer.query_proposals().await.map_err(|err| {
            log::warn!(
                "[hotstuff] local: {:?}, generate_block query proposal failed {:?}",
                self,
                err
            );
            err
        })?;

        proposals.sort_by(|left, right| left.desc().create_time().cmp(&right.desc().create_time()));

        let prev_block = match self.high_qc.as_ref() {
            Some(qc) => Some(self.store.find_block_in_cache(&qc.block_id)?),
            None => None,
        };
        let latest_group = self.committee.get_group(None).await.map_err(|err| {
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
            if Self::calc_time_delta(now, create_time)
                > TIME_PRECISION
            {
                // 时间误差太大
                remove_proposals.push(proposal.desc().object_id());
                time_adjust_proposals.push(proposal);
                continue;
            }

            let ending =  proposal.effective_ending()
                .map_or(now.checked_add(PROPOSAL_MAX_TIMEOUT).unwrap(), 
                    |ending| bucky_time_to_system_time(ending));
            if now >= ending {
                remove_proposals.push(proposal.desc().object_id());
                timeout_proposals.push(proposal);
                continue;
            }

            match self.delegate.on_execute(&proposal, result_state_id).await {
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

        self.notify_adjust_time_proposals(time_adjust_proposals).await;
        self.notify_timeout_proposals(timeout_proposals).await;
        self.notify_failed_proposals(failed_proposals).await;
        self.remove_pending_proposals(remove_proposals).await;

        if self
            .try_wait_proposals(executed_proposals.as_slice(), &prev_block)
            .await
        {
            log::debug!(
                "[hotstuff] local: {:?}, generate_block empty block, will ignore",
                self,
            );
            return Ok(());
        }

        let proposals_map = HashMap::from_iter(
            executed_proposals.iter()
                .map(|(proposal, _)| (proposal.desc().object_id(), proposal.clone()))
        );

        let block = self.package_block_with_proposals(executed_proposals, &latest_group, result_state_id, &prev_block, tc).await?;

        self.broadcast(HotstuffMessage::Block(block.clone()), &latest_group);
        self.tx_block_gen.send((block, proposals_map)).await;

        self.rx_proposal_waiter = None;
        Ok(())
    }

    async fn notify_adjust_time_proposals(&self, time_adjust_proposals: Vec<GroupProposal>) {
        if time_adjust_proposals.len() > 0 {
            log::warn!(
                "[hotstuff] local: {:?}, generate_block timestamp err {:?}",
                self,
                time_adjust_proposals.iter().map(|proposal| {
                    let desc = proposal.desc();
                    (desc.object_id(), desc.owner(), bucky_time_to_system_time(desc.create_time()))
                }).collect::<Vec<_>>()
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
                timeout_proposals.iter().map(|proposal| {
                    let desc = proposal.desc();
                    (
                        desc.object_id(),
                        desc.owner(),
                        bucky_time_to_system_time(desc.create_time()),
                        proposal.effective_ending().as_ref().map(|ending| bucky_time_to_system_time(*ending))
                    )
                }).collect::<Vec<_>>()
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
                failed_proposals.iter().map(|(proposal, err)| {
                    let desc = proposal.desc();
                    (
                        desc.object_id(),
                        desc.owner(),
                        err.clone()
                    )
                }).collect::<Vec<_>>()
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

        self.proposal_consumer
            .remove_proposals(pending_proposals)
            .await;
    }

    async fn package_block_with_proposals(&self,
        executed_proposals: Vec<(GroupProposal, ExecuteResult)>,
        group: &Group,
        result_state_id: Option<ObjectId>,
        prev_block: &Option<GroupConsensusBlock>,
        tc: Option<HotstuffTimeout>
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

        let group_chunk_id = ChunkMeta::from(group)
            .to_chunk()
            .await
            .unwrap()
            .calculate_id();

        let mut block = GroupConsensusBlock::create(
            self.rpath.clone(),
            proposals_param,
            result_state_id,
            prev_block.as_ref().map_or(0, |b| b.height()) + 1,
            ObjectId::default(), // TODO: meta block id
            self.round,
            group_chunk_id.object_id(),
            self.high_qc.clone(),
            tc,
            self.local_id,
        );

        log::info!(
            "[hotstuff] local: {:?}, generate_block new block {}/{}/{}, with proposals: {}",
            self,
            block.block_id(), block.height(), block.round(), proposal_count
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

    fn broadcast(&self, msg: HotstuffMessage, group: &Group) -> BuckyResult<()> {
        let targets: Vec<ObjectId> = group
            .ood_list()
            .iter()
            .filter(|ood_id| **ood_id != self.local_device_id)
            .map(|ood_id| ood_id.object_id().clone())
            .collect();

        let network_sender = self.network_sender.clone();
        let rpath = self.rpath.clone();

        async_std::task::spawn(async move {
            network_sender.broadcast(msg, rpath.clone(), targets.as_slice()).await
        });

        Ok(())
    }

    async fn try_wait_proposals(
        &mut self,
        executed_proposals: &[(GroupProposal, ExecuteResult)],
        pre_block: &Option<GroupConsensusBlock>,
    ) -> bool {
        // empty block, qc only, it's unuseful when no block to qc
        let mut will_wait_proposals = false;
        if executed_proposals.len() == 0 {
            match pre_block.as_ref() {
                None => {
                    log::warn!(
                        "[hotstuff] local: {:?}, new empty block will ignore for first block is empty.",
                        self,
                    );

                    will_wait_proposals = true
                },
                Some(pre_block) => {
                    if pre_block.proposals().len() == 0 {
                        match pre_block.prev_block_id() {
                            Some(pre_pre_block_id) => {
                                let pre_pre_block =
                                    match self.store.find_block_in_cache(pre_pre_block_id) {
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
                _ => return false
            }
        }

        will_wait_proposals
    }

    async fn handle_proposal_waiting(&mut self) -> BuckyResult<()> {
        log::debug!(
            "[hotstuff] local: {:?}, handle_proposal_waiting",
            self
        );

        assert_eq!(self.committee.get_leader(None, self.round).await?, self.local_device_id);

        let tc = self.tc.as_ref().map_or(None, |tc| {
            if tc.round + 1 == self.round {
                Some(tc.clone())
            } else {
                None
            }
        });
        self.generate_block(tc).await
    }

    async fn fetch_block(&mut self, block_id: &ObjectId, remote: ObjectId) -> BuckyResult<()> {
        let block = self.non_driver.get_block(block_id, Some(&remote)).await?;

        self.tx_message
            .send((HotstuffMessage::Block(block), remote))
            .await;
        Ok(())
    }

    async fn check_group_is_latest(&self, group_chunk_id: &ObjectId) -> BuckyResult<bool> {
        let latest_group = self.committee.get_group(None).await?;
        let group_chunk = ChunkMeta::from(&latest_group).to_chunk().await?;
        let latest_chunk_id = group_chunk.calculate_id();
        Ok(latest_chunk_id.as_object_id() == group_chunk_id)
    }

    async fn recover(&mut self) {
        // Upon booting, generate the very first block (if we are the leader).
        // Also, schedule a timer in case we don't hear from the leader.
        let max_round_block = self.store.block_with_max_round();
        let group_chunk_id = max_round_block.as_ref().map(|block| block.group_chunk_id());
        let last_group = self.committee.get_group(group_chunk_id).await;
        let latest_group = match group_chunk_id.as_ref() {
            Some(_) => self.committee.get_group(None).await,
            None => last_group.clone(),
        };

        let duration = latest_group
            .as_ref()
            .map_or(HOTSTUFF_TIMEOUT_DEFAULT, |g| g.consensus_interval());
        self.timer.reset(duration);

        if let Ok(leader) = self.committee.get_leader(None, self.round).await {
            if leader == self.local_device_id {
                match max_round_block {
                    Some(max_round_block)
                        if max_round_block.owner() == &self.local_id
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
                        self.generate_block(None).await;
                    }
                }
            }
        }
    }

    fn proposal_waiter(waiter: Option<(Receiver<()>, u64)>) -> impl futures::Future<Output = u64> {
        async move {
            match waiter.as_ref() {
                Some((waiter, wait_round)) => {
                    waiter.recv().await;
                    *wait_round
                }
                None => std::future::pending::<u64>().await,
            }
        }
    }

    async fn run(&mut self) -> ! {
        self.recover().await;

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
                    Ok((HotstuffMessage::QueryState(_), _)) => panic!("should process by DecStateRequestor"),
                    Ok((HotstuffMessage::VerifiableState(_, _), _)) => panic!("should process by DecStateRequestor"),
                    Err(e) => {
                        log::warn!("[hotstuff] rx_message closed.");
                        Ok(())
                    },
                },
                block = self.rx_block_gen.recv().fuse() => match block {
                    Ok((block, proposals)) => self.process_block(&block, self.local_device_id, &proposals).await,
                    Err(e) => {
                        log::warn!("[hotstuff] rx_block_gen closed.");
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
