use std::{collections::HashMap, sync::Arc};

use async_std::channel::{Receiver, Sender};
use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, NamedObject, ObjectDesc, ObjectId, RsaCPUObjectSigner,
};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal, GroupRPath, HotstuffBlockQC,
    HotstuffTimeout,
};
use cyfs_lib::NONObjectInfo;

use crate::{
    consensus::{order_block::OrderBlockMgr, timer::Timer},
    Committee, ExecuteResult, HotstuffBlockQCVote, HotstuffMessage, HotstuffTimeoutVote,
    PendingProposalMgr, ProposalConsumeMessage, RPathDelegate, Storage, VoteMgr,
};

pub struct Hotstuff {
    local_id: ObjectId,
    committee: Committee,
    store: Storage,
    signer: RsaCPUObjectSigner,
    round: u64,               // 当前轮次
    high_qc: HotstuffBlockQC, // 最后一次通过投票的确认信息
    timer: Timer,             // 定时器
    vote_mgr: VoteMgr,
    network_sender: crate::network::Sender,
    non_driver: crate::network::NonDriver,
    rx_message: Receiver<HotstuffMessage>,
    tx_proposal_consume: Sender<ProposalConsumeMessage>,
    delegate: Arc<Box<dyn RPathDelegate>>,
    order_block_mgr: OrderBlockMgr,
    rpath: GroupRPath,
}

impl Hotstuff {
    pub fn spawn(
        local_id: ObjectId,
        committee: Committee,
        store: Storage,
        signer: RsaCPUObjectSigner,
        vote_mgr: VoteMgr,
        network_sender: crate::network::Sender,
        non_driver: crate::network::NonDriver,
        rx_message: Receiver<HotstuffMessage>,
        tx_proposal_consume: Sender<ProposalConsumeMessage>,
        delegate: Arc<Box<dyn RPathDelegate>>,
        rpath: GroupRPath,
    ) {
        let mut round = 0;
        let mut high_qc = HotstuffBlockQC::default();

        for block in store.prepares().values() {
            if block.round() > round {
                round = block.round();
            }

            if block.qc().round > high_qc.round {
                high_qc = block.qc().clone();
            }
        }

        for block in store.pre_commits().values() {
            if block.round() > round {
                round = block.round();
            }

            if block.qc().round > high_qc.round {
                high_qc = block.qc().clone();
            }
        }

        let obj = Self {
            local_id,
            committee,
            store,
            signer,
            round,
            high_qc,
            timer: Timer::new(store.group().consensus_interval()),
            vote_mgr,
            network_sender,
            rx_message,
            delegate,
            order_block_mgr: OrderBlockMgr::new(),
            non_driver,
            rpath,
            tx_proposal_consume,
        };
    }

    // TODO: 网络层应该防御，只从当前group节点获取信息
    async fn handle_block(&mut self, block: &GroupConsensusBlock) -> BuckyResult<()> {
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
                .get_leader(block.group_chunk_id(), block.round())
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
                    return self.order_block_mgr.push_block(block).await;
                }
            }
        };

        self.committee.verify_block(block).await?;

        self.check_block_proposal_result_state_by_app(block, &proposals, &prev_block)
            .await?;

        self.order_block_mgr.pop_link(block).await?;

        self.process_qc(block.qc()).await;

        if let Some(tc) = block.tc() {
            self.advance_round(tc.round).await;
        }

        self.process_block(block).await
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

    async fn process_block(&mut self, block: &GroupConsensusBlock) -> BuckyResult<()> {
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
                self.notify_proposal_result(&header_block);
            }
        }

        if block.round() != self.round {
            // 不是我的投票round
            return Ok(());
        }

        if let Some(vote) = self.make_vote(block).await {
            let next_leader = self
                .committee
                .get_leader(block.group_chunk_id(), self.round + 1)
                .await?;

            if self.local_id == next_leader {
                self.handle_vote(&vote).await?;
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

    async fn process_qc(&mut self, qc: &HotstuffBlockQC) {
        self.advance_round(qc.round).await;
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
        }
    }

    fn update_high_qc(&mut self, qc: &HotstuffBlockQC) {
        if qc.round > self.high_qc.round {
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

    fn notify_proposal_result(&self, block: &GroupConsensusBlock) {
        // 通知客户端proposal执行结果
        unimplemented!()
    }

    async fn make_vote(&mut self, block: &GroupConsensusBlock) -> Option<HotstuffBlockQCVote> {
        if block.round() <= self.store.last_vote_round() {
            return None;
        }

        // round只能逐个递增
        let is_valid_round = if block.round() == block.qc().round + 1 {
            true
        } else if let Some(tc) = block.tc() {
            block.round() == tc.round + 1
                && block.qc().round >= tc.votes.iter().map(|v| v.high_qc_round).max().unwrap()
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

    async fn handle_vote(&mut self, vote: &HotstuffBlockQCVote) -> BuckyResult<()> {
        if vote.round < self.round {
            return Err(BuckyError::new(BuckyErrorCode::Expired, "expired vote"));
        }

        self.committee.verify_vote(vote).await?;

        if let Some(qc) = self.vote_mgr.add_vote(vote.clone())? {
            self.process_qc(&qc).await;

            if self.local_id == self.committee.get_leader(group_chunk_id, round)
        }
    }

    async fn handle_timeout(&mut self, timeout: &HotstuffTimeoutVote) -> BuckyResult<()> {
        unimplemented!()
    }

    async fn handle_tc(&mut self, tc: &HotstuffTimeout) -> BuckyResult<()> {
        unimplemented!()
    }

    async fn local_timeout_round(&mut self) -> BuckyResult<()> {
        unimplemented!()
    }

    async fn run(&mut self) {
        // Upon booting, generate the very first block (if we are the leader).
        // Also, schedule a timer in case we don't hear from the leader.
        self.timer.reset(
            self.committee
                .get_group(self.header_block())
                .await
                .map_or(HOTSTUFF_TIMEOUT_DEFAULT, |g| g.consensus_interval()),
        );
        if let Ok(leader) = self
            .committee
            .get_next_leader(self.store.header_block())
            .await
        {
            if leader == self.local_id {
                self.generate_proposal(None).await;
            }
        }

        // This is the main loop: it processes incoming blocks and votes,
        // and receive timeout notifications from our Timeout Manager.
        loop {
            let result = futures::select! {
                Some(message) = self.rx_message.recv() => match message {
                    HotstuffMessage::Block(block) => self.handle_block(&block).await,
                    HotstuffMessage::BlockVote(vote) => self.handle_vote(&vote).await,
                    HotstuffMessage::TimeoutVote(timeout) => self.handle_timeout(&timeout).await,
                    HotstuffMessage::Timeout(tc) => self.handle_tc(&tc).await,
                    _ => panic!("Unexpected protocol message")
                },
                Some(block) = self.rx_loopback.recv() => self.process_block(&block).await,
                () = &mut self.timer => self.local_timeout_round().await,
            };
        }
    }
}
