use std::{collections::HashMap, sync::Arc, time::SystemTime};

use async_std::channel::{Receiver, Sender};
use cyfs_base::{
    bucky_time_to_system_time, BuckyError, BuckyErrorCode, BuckyResult, ChunkId, Group,
    NamedObject, ObjectDesc, ObjectId, RsaCPUObjectSigner,
};
use cyfs_chunk_lib::ChunkMeta;
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, GroupConsensusBlockProposal, GroupProposal,
    GroupProposalObject, GroupRPath, HotstuffBlockQC, HotstuffTimeout,
};
use cyfs_lib::NONObjectInfo;
use futures::FutureExt;

use crate::{
    consensus::{synchronizer::Synchronizer, timer::Timer},
    AsProposal, Committee, ExecuteResult, HotstuffBlockQCVote, HotstuffMessage,
    HotstuffTimeoutVote, PendingProposalMgr, ProposalConsumeMessage, RPathDelegate, Storage,
    VoteMgr, VoteThresholded, CHANNEL_CAPACITY, HOTSTUFF_TIMEOUT_DEFAULT, TIME_PRECISION,
};

/**
 * TODO: generate empty block when the 'Node' is synchronizing
*/

pub struct Hotstuff {
    local_id: ObjectId,
    committee: Committee,
    store: Storage,
    signer: RsaCPUObjectSigner,
    round: u64,                       // 当前轮次
    high_qc: Option<HotstuffBlockQC>, // 最后一次通过投票的确认信息
    tc: Option<HotstuffTimeout>,
    timer: Timer, // 定时器
    vote_mgr: VoteMgr,
    network_sender: crate::network::Sender,
    non_driver: crate::network::NonDriver,
    rx_message: Receiver<(HotstuffMessage, ObjectId)>,
    tx_proposal_consume: Sender<ProposalConsumeMessage>,
    delegate: Arc<Box<dyn RPathDelegate>>,
    synchronizer: Synchronizer,
    rpath: GroupRPath,
    tx_message_inner: Sender<(GroupConsensusBlock, ObjectId)>,
    rx_message_inner: Receiver<(GroupConsensusBlock, ObjectId)>,
    rx_proposal_waiter: Option<(Receiver<u64>, u64)>,
}

impl Hotstuff {
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        local_id: ObjectId,
        committee: Committee,
        store: Storage,
        signer: RsaCPUObjectSigner,
        network_sender: crate::network::Sender,
        non_driver: crate::network::NonDriver,
        rx_message: Receiver<(HotstuffMessage, ObjectId)>,
        tx_proposal_consume: Sender<ProposalConsumeMessage>,
        delegate: Arc<Box<dyn RPathDelegate>>,
        rpath: GroupRPath,
    ) {
        let max_round_block = store.block_with_max_round();

        let round = store
            .last_vote_round()
            .max(max_round_block.as_ref().map_or(1, |block| block.round()));
        let high_qc = max_round_block.map_or(None, |block| block.qc().clone());

        let (tx_message_inner, rx_message_inner) = async_std::channel::bounded(CHANNEL_CAPACITY);

        let vote_mgr = VoteMgr::new(committee.clone(), round);
        let init_timer_interval = store.group().consensus_interval();

        let mut hotstuff = Self {
            local_id,
            committee,
            store,
            signer,
            round,
            high_qc,
            timer: Timer::new(init_timer_interval),
            vote_mgr,
            network_sender,
            rx_message,
            delegate,
            synchronizer: Synchronizer::spawn(network_sender.clone(), rpath.clone(), rx_message),
            non_driver,
            rpath,
            tx_proposal_consume,
            tx_message_inner,
            rx_message_inner,
            rx_proposal_waiter: None,
            tc: None,
        };

        async_std::task::spawn(async move { hotstuff.run().await });
    }

    async fn handle_block(
        &mut self,
        block: &GroupConsensusBlock,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        let latest_group = self.committee.get_group(None).await?;
        if !latest_group.contain_ood(&remote) {
            log::warn!(
                "receive block({}) from unknown({})",
                block.named_object().desc().object_id(),
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

        {
            // check leader
            let leader = self
                .committee
                .get_leader(Some(block.group_chunk_id()), block.round())
                .await?;
            if &leader != block.owner() {
                log::warn!(
                    "receive block from invalid leader({}), expected {}",
                    block.owner(),
                    leader
                );
                return Err(BuckyError::new(BuckyErrorCode::Ignored, "invalid leader"));
            }
        }

        let (prev_block, proposals) = {
            match self.store.block_linked(block).await? {
                crate::storage::BlockLinkState::Expired => {
                    log::warn!("receive block expired.");
                    return Err(BuckyError::new(BuckyErrorCode::Ignored, "expired"));
                }
                crate::storage::BlockLinkState::DuplicateProposal => {
                    log::warn!("receive block with duplicate proposal.");
                    return Err(BuckyError::new(
                        BuckyErrorCode::AlreadyExists,
                        "duplicate proposal",
                    ));
                }
                crate::storage::BlockLinkState::Duplicate => {
                    log::warn!("receive duplicate block.");
                    return Err(BuckyError::new(
                        BuckyErrorCode::AlreadyExists,
                        "duplicate block",
                    ));
                }
                crate::storage::BlockLinkState::Link(prev_block, proposals) => {
                    // 顺序连接状态
                    Self::check_empty_block_result_state_with_prev(block, &prev_block)?;
                    (prev_block, proposals)
                }
                crate::storage::BlockLinkState::Pending => {
                    // 乱序，同步
                    if block.height() <= self.store.header_height() + 3 {
                        self.fetch_block(block.prev_block_id().unwrap(), remote)
                            .await;
                    }

                    let max_round_block = self.store.block_with_max_round();
                    return self.synchronizer.push_outorder_block(
                        block.clone(),
                        max_round_block.map_or(1, |block| block.height() + 1),
                        remote,
                    );
                }
                crate::storage::BlockLinkState::InvalidBranch => {
                    log::warn!("receive block in invalid branch.");
                    return Err(BuckyError::new(BuckyErrorCode::Conflict, "conflict branch"));
                }
            }
        };

        self.committee.verify_block(block).await?;

        self.check_block_proposal_result_state_by_app(block, &proposals, &prev_block)
            .await?;

        self.synchronizer.pop_link_from(block)?;

        self.process_qc(block.qc()).await;

        if let Some(tc) = block.tc() {
            self.advance_round(tc.round).await;
        }

        self.process_block(block, remote).await
    }

    fn check_block_result_state(block: &GroupConsensusBlock) -> BuckyResult<()> {
        if let Some(last_proposal) = block.proposals().last() {
            if &last_proposal.result_state != block.result_state_id() {
                log::warn!("the result-state({}) in last-proposal is unmatch with block.result_state_id({})"
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
                        log::warn!("block.result_state_id({}) is unmatch with prev_block.result_state_id({}) with no proposal."
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
            .map_or(ObjectId::default(), |block| block.result_state_id().clone());

        for proposal_exe_info in block.proposals() {
            // TODO: 去重
            let proposal = proposals.get(&proposal_exe_info.proposal).unwrap();
            let receipt = if proposal_exe_info.receipt.len() > 0 {
                Some(NONObjectInfo::new_from_object_raw(
                    proposal_exe_info.receipt.clone(),
                )?)
            } else {
                None
            };

            let exe_result = ExecuteResult {
                result_state_id: proposal_exe_info.result_state,
                receipt,
                context: proposal_exe_info.context.clone(),
            };

            if self
                .delegate
                .on_verify(proposal, prev_state_id, &exe_result)
                .await?
            {
                prev_state_id = proposal_exe_info.result_state;
            } else {
                log::warn!(
                    "block verify failed by app, proposal: {}, prev_state: {}, expect-result: {}",
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

    async fn process_block(
        &mut self,
        block: &GroupConsensusBlock,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        /**
         * 验证过的块执行这个函数
         */
        let new_header_block = self.store.push_block(block.clone()).await?;

        if let Some(header_block) = new_header_block.map(|b| b.0.clone()) {
            /**
             * TODO:
             * 这里只清理已经提交的block包含的proposal
             * 已经执行过的待提交block包含的proposal在下次打包时候去重
             * */
            self.cleanup_proposal(&header_block).await;

            if block.owner() == &self.local_id {
                self.notify_proposal_result_for_block(&header_block);
            }
        }

        match self.vote_mgr.add_voting_block(block).await {
            VoteThresholded::QC(qc) => return self.process_block_qc(qc, block, remote).await,
            VoteThresholded::TC(tc, max_high_qc_block) => {
                return self
                    .process_timeout_qc(tc, max_high_qc_block.as_ref())
                    .await
            }
            VoteThresholded::None => {}
        }

        if block.round() != self.round {
            // 不是我的投票round
            return Ok(());
        }

        if let Some(vote) = self.make_vote(block).await {
            let next_leader = self.committee.get_leader(None, self.round + 1).await?;

            if self.local_id == next_leader {
                self.handle_vote(&vote, Some(block), remote).await?;
            } else {
                self.network_sender
                    .post_package(
                        HotstuffMessage::BlockVote(vote),
                        self.rpath.clone(),
                        &next_leader,
                    )
                    .await;
            }
        }

        Ok(())
    }

    async fn process_qc(&mut self, qc: &Option<HotstuffBlockQC>) {
        self.advance_round(qc.as_ref().map_or(0, |qc| qc.round))
            .await;
        self.update_high_qc(qc);
    }

    async fn advance_round(&mut self, round: u64) {
        if round < self.round {
            return;
        }

        if let Ok(group) = self.committee.get_group(None).await {
            self.timer.reset(group.consensus_interval());
            self.round = round + 1;
            self.vote_mgr.cleanup(self.round);
            self.tc = None;
        }
    }

    fn update_high_qc(&mut self, qc: &Option<HotstuffBlockQC>) {
        if qc.as_ref().map_or(0, |qc| qc.round) > self.high_qc.as_ref().map_or(0, |qc| qc.round) {
            self.high_qc = qc.clone();
        }
    }

    async fn cleanup_proposal(&mut self, commited_block: &GroupConsensusBlock) -> BuckyResult<()> {
        let proposals = commited_block
            .proposals()
            .iter()
            .map(|proposal| proposal.proposal)
            .collect();
        PendingProposalMgr::remove_proposals(&self.tx_proposal_consume, proposals).await
    }

    fn notify_proposal_result(&self, proposal: &GroupProposal, result: &NONObjectInfo) {
        // 通知客户端proposal执行结果
        unimplemented!()
    }

    fn notify_proposal_result_for_block(&self, block: &GroupConsensusBlock) {
        unimplemented!()
    }

    async fn make_vote(&mut self, block: &GroupConsensusBlock) -> Option<HotstuffBlockQCVote> {
        if block.round() <= self.store.last_vote_round() {
            return None;
        }

        // round只能逐个递增
        let is_valid_round = if block.round() == block.qc().as_ref().map_or(0, |qc| qc.round) + 1 {
            true
        } else if let Some(tc) = block.tc() {
            block.round() == tc.round + 1
                && block.qc().as_ref().map_or(0, |qc| qc.round)
                    >= tc.votes.iter().map(|v| v.high_qc_round).max().unwrap()
        } else {
            false
        };

        if !is_valid_round {
            return None;
        }

        let vote = match HotstuffBlockQCVote::new(block, self.local_id, &self.signer).await {
            Ok(vote) => vote,
            Err(e) => {
                log::warn!(
                    "signature for block-vote failed, block: {}, err: {}",
                    block.named_object().desc().object_id(),
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

    async fn handle_vote(
        &mut self,
        vote: &HotstuffBlockQCVote,
        block: Option<&GroupConsensusBlock>,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        if vote.round < self.round {
            return Ok(());
        }

        self.committee.verify_vote(vote).await?;

        let block = match block {
            Some(b) => Some(b.clone()),
            None => self
                .store
                .find_block_in_cache(&vote.block_id)
                .await
                .map_or(None, |b| Some(b)),
        };

        if let Some((qc, block)) = self.vote_mgr.add_vote(vote.clone(), block).await? {
            self.process_block_qc(qc, &block, remote).await?;
        } else if vote.round > self.round {
            self.fetch_block(&vote.block_id, remote).await;
        }

        Ok(())
    }

    async fn process_block_qc(
        &mut self,
        qc: HotstuffBlockQC,
        block: &GroupConsensusBlock,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        self.process_qc(&Some(qc)).await;

        if self.local_id == self.committee.get_leader(None, self.round).await? {
            self.generate_proposal(None).await;
        }
        Ok(())
    }

    async fn handle_timeout(
        &mut self,
        timeout: &HotstuffTimeoutVote,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        if timeout.round < self.round
            || timeout.high_qc.as_ref().map_or(0, |qc| qc.round) >= timeout.round
        {
            return Ok(());
        }

        let block = match timeout.high_qc.as_ref() {
            Some(qc) => match self.store.find_block_in_cache(&qc.block_id).await {
                Ok(block) => Some(block),
                Err(_) => {
                    self.vote_mgr.add_waiting_timeout(timeout.clone());
                    self.fetch_block(&qc.block_id, remote).await;
                    return Ok(());
                }
            },
            None => None,
        };

        self.committee
            .verify_timeout(timeout, block.as_ref())
            .await?;

        self.process_qc(&timeout.high_qc).await;

        if let Some((tc, max_high_qc_block)) = self
            .vote_mgr
            .add_timeout(timeout.clone(), block.as_ref())
            .await?
        {
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
        self.advance_round(tc.round).await;
        self.tc = Some(tc.clone());

        if self.local_id == self.committee.get_leader(None, self.round).await? {
            self.generate_proposal(Some(tc)).await;
            Ok(())
        } else {
            let latest_group = self.committee.get_group(None).await?;
            self.broadcast(HotstuffMessage::Timeout(tc), &latest_group)
                .await
        }
    }

    async fn handle_tc(&mut self, tc: &HotstuffTimeout, remote: ObjectId) -> BuckyResult<()> {
        let max_high_qc = tc
            .votes
            .iter()
            .max_by(|high_qc_l, high_qc_r| high_qc_l.high_qc_round.cmp(&high_qc_r.high_qc_round));

        let max_high_qc = if let Some(max_high_qc) = max_high_qc {
            max_high_qc
        } else {
            return Ok(());
        };

        if tc.round < self.round || max_high_qc.high_qc_round >= tc.round {
            return Ok(());
        }

        let block = if max_high_qc.high_qc_round == 0 {
            None
        } else {
            let block = match self
                .store
                .find_block_in_cache_by_round(max_high_qc.high_qc_round)
                .await
            {
                Ok(block) => block,
                Err(_) => {
                    // 同步前序block
                    let max_round_block = self.store.block_with_max_round();
                    return self.synchronizer.sync_with_round(
                        max_round_block.map_or(1, |block| block.height() + 1),
                        max_high_qc.high_qc_round,
                        remote,
                    );
                }
            };
            Some(block)
        };

        self.committee.verify_tc(tc, block.as_ref()).await?;

        self.advance_round(tc.round).await;
        self.tc = Some(tc.clone());

        if self.local_id == self.committee.get_leader(None, self.round).await? {
            self.generate_proposal(Some(tc.clone())).await;
        }
        Ok(())
    }

    async fn local_timeout_round(&mut self) -> BuckyResult<()> {
        let latest_group = match self.committee.get_group(None).await {
            Ok(group) => {
                self.timer.reset(group.consensus_interval());
                group
            }
            Err(e) => {
                self.timer.reset(HOTSTUFF_TIMEOUT_DEFAULT);
                return Err(e);
            }
        };

        match self.tc.as_ref() {
            Some(tc) => {
                if tc.round + 1 == self.round {
                    self.broadcast(HotstuffMessage::Timeout(tc.clone()), &latest_group)
                        .await;
                }
            }
            None => {
                let timeout = HotstuffTimeoutVote::new(
                    self.high_qc.clone(),
                    self.round,
                    self.local_id,
                    &self.signer,
                )
                .await?;

                self.store.set_last_vote_round(self.round).await?;

                self.handle_timeout(&timeout, self.local_id).await;

                self.broadcast(HotstuffMessage::TimeoutVote(timeout), &latest_group)
                    .await;
            }
        }

        Ok(())
    }

    async fn generate_proposal(&mut self, tc: Option<HotstuffTimeout>) -> BuckyResult<()> {
        let mut proposals = PendingProposalMgr::query_proposals(&self.tx_proposal_consume).await?;
        proposals.sort_by(|left, right| left.desc().create_time().cmp(&right.desc().create_time()));

        let now = SystemTime::now();

        let pre_block = match self.high_qc.as_ref() {
            Some(qc) => Some(self.store.find_block_in_cache(&qc.block_id).await?),
            None => None,
        };
        let latest_group = self.committee.get_group(None).await?;

        let mut remove_proposals = vec![];
        // let mut dup_proposals = vec![];
        let mut time_adjust_proposals = vec![];
        let mut timeout_proposals = vec![];
        let mut executed_proposals = vec![];
        let mut failed_proposals = vec![];
        let mut result_state_id = match pre_block.as_ref() {
            Some(block) => block.result_state_id().clone(),
            None => self.store.dec_state_id(),
        };

        // TODO: The time may be too long for too many proposals
        for proposal in proposals {
            if let Some(high_qc) = self.high_qc.as_ref() {
                if let Ok(is_finished) = self
                    .store
                    .is_proposal_finished(&proposal.id(), &high_qc.block_id)
                    .await
                {
                    if is_finished {
                        // dup_proposals.push(proposal);
                        remove_proposals.push(proposal.id());
                        continue;
                    }
                }
            }

            let create_time = bucky_time_to_system_time(proposal.desc().create_time());
            if now
                .duration_since(create_time)
                .or(create_time.duration_since(now))
                .unwrap()
                > TIME_PRECISION
            {
                // 时间误差太大
                remove_proposals.push(proposal.id());
                time_adjust_proposals.push(proposal);
                continue;
            }

            if let Some(ending) = proposal.effective_ending() {
                if now >= bucky_time_to_system_time(ending) {
                    remove_proposals.push(proposal.id());
                    timeout_proposals.push(proposal);
                    continue;
                }
            }

            match self.delegate.on_execute(&proposal, result_state_id).await {
                Ok(exe_result) => {
                    result_state_id = exe_result.result_state_id;
                    executed_proposals.push((proposal, exe_result));
                }
                Err(e) => {
                    remove_proposals.push(proposal.id());
                    failed_proposals.push((proposal, e));
                }
            };
        }

        for proposal in time_adjust_proposals {
            // TODO: 矫正系统时间
            // self.notify_proposal_result(&proposal, result);
        }

        for proposal in timeout_proposals {
            // TODO: 超时
            // self.notify_proposal_result(&proposal, result);
        }

        for proposal in failed_proposals {
            // TODO: 执行失败
            // self.notify_proposal_result(&proposal, result)
        }

        PendingProposalMgr::remove_proposals(&self.tx_proposal_consume, remove_proposals).await;

        if self
            .try_wait_proposals(executed_proposals.as_slice(), &pre_block)
            .await
        {
            return Ok(());
        }

        let proposals_param = executed_proposals
            .into_iter()
            .map(|(proposal, exe_result)| GroupConsensusBlockProposal {
                proposal: proposal.id(),
                result_state: exe_result.result_state_id,
                receipt: exe_result
                    .receipt
                    .map_or(vec![], |receipt| receipt.object_raw),
                context: exe_result.context,
            })
            .collect();

        let group_chunk_id = ChunkMeta::from(&latest_group)
            .to_chunk()
            .await
            .unwrap()
            .calculate_id();

        let block = GroupConsensusBlock::create(
            self.rpath.clone(),
            proposals_param,
            result_state_id,
            self.store.header_height(),
            ObjectId::default(), // TODO: meta block id
            self.round,
            group_chunk_id.object_id(),
            self.high_qc.clone(),
            tc,
            self.local_id,
        );

        self.tx_message_inner
            .send((block.clone(), self.local_id))
            .await;

        self.broadcast(HotstuffMessage::Block(block), &latest_group)
            .await;

        self.rx_proposal_waiter = None;
        Ok(())
    }

    async fn broadcast(&self, msg: HotstuffMessage, group: &Group) -> BuckyResult<()> {
        let targets: Vec<ObjectId> = group
            .ood_list()
            .iter()
            .filter(|ood_id| **ood_id != self.local_id)
            .map(|ood_id| ood_id.object_id().clone())
            .collect();

        self.network_sender
            .broadcast(msg, self.rpath.clone(), targets.as_slice())
            .await;

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
                None => will_wait_proposals = true,
                Some(pre_block) => {
                    if pre_block.proposals().len() == 0 {
                        match pre_block.prev_block_id() {
                            Some(pre_pre_block_id) => {
                                let pre_pre_block =
                                    match self.store.find_block_in_cache(pre_pre_block_id).await {
                                        Ok(pre_pre_block) => pre_pre_block,
                                        Err(_) => return false,
                                    };
                                if pre_pre_block.proposals().len() == 0 {
                                    will_wait_proposals = true;
                                }
                            }
                            None => will_wait_proposals = true,
                        }
                    }
                }
            }
        }

        if will_wait_proposals {
            let (tx, rx) = async_std::channel::bounded(1);
            self.rx_proposal_waiter = Some((rx, self.round));
        }

        will_wait_proposals
    }

    async fn fetch_block(&mut self, block_id: &ObjectId, remote: ObjectId) -> BuckyResult<()> {
        let block = self.non_driver.get_block(block_id, Some(&remote)).await?;

        self.tx_message_inner.send((block, remote)).await;
        Ok(())
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
            if leader == self.local_id {
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
                        )
                        .await;
                    }
                    _ => {
                        self.generate_proposal(None).await;
                    }
                }
            }
        }
    }

    fn proposal_waiter(waiter: Option<(Receiver<u64>, u64)>) -> impl futures::Future<Output = u64> {
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

    async fn run(&mut self) {
        self.recover().await;

        // This is the main loop: it processes incoming blocks and votes,
        // and receive timeout notifications from our Timeout Manager.
        loop {
            let result = futures::select! {
                message = self.rx_message.recv().fuse() => match message {
                    Ok((HotstuffMessage::Block(block), remote)) => self.handle_block(&block, remote).await,
                    Ok((HotstuffMessage::BlockVote(vote), remote)) => self.handle_vote(&vote, None, remote).await,
                    Ok((HotstuffMessage::TimeoutVote(timeout), remote)) => self.handle_timeout(&timeout, remote).await,
                    Ok((HotstuffMessage::Timeout(tc), remote)) => self.handle_tc(&tc, remote).await,
                    Err(e) => {
                        log::warn!("[hotstuff] rx_message closed.");
                        Ok(())
                    },
                    _ => panic!("unknown message")
                },
                message = self.rx_message_inner.recv().fuse() => match message {
                    Ok((block, remote)) => {
                        if remote == self.local_id {
                            self.process_block(&block, remote).await
                        } else {
                            self.handle_block(&block, remote).await
                        }
                    }
                    Err(e) => {
                        log::warn!("[hotstuff] rx_message_inner closed.");
                        Ok(())
                    }
                },
                () = self.timer.wait_next().fuse() => self.local_timeout_round().await,
                wait_round = Self::proposal_waiter(self.rx_proposal_waiter.clone()).fuse() => {
                    if wait_round == self.round {
                        self.generate_proposal(None).await
                    } else {
                        Ok(())
                    }
                }
            };
        }
    }
}
