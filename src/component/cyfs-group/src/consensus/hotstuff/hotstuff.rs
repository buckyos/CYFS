use std::sync::Arc;

use async_std::channel::{Receiver, Sender};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId, RsaCPUObjectSigner};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, GroupRPath, HotstuffBlockQC, HotstuffTimeout,
};
use cyfs_lib::NONObjectInfo;

use crate::{
    consensus::{order_block::OrderBlockMgr, proposal, timer::Timer},
    Committee, ExecuteResult, HotstuffBlockQCVote, HotstuffMessage, HotstuffTimeoutVote,
    RPathDelegate, Storage, VoteMgr,
};

pub struct Hotstuff {
    local_id: ObjectId,
    committee: Committee,
    store: Storage,
    signer: RsaCPUObjectSigner,
    round: u64,                // 当前轮次
    last_committed_round: u64, // 最后提交的轮次
    high_qc: HotstuffBlockQC,  // 最后一次通过投票的确认信息
    timer: Timer,              // 定时器
    vote_mgr: VoteMgr,
    network_sender: crate::network::Sender,
    non_driver: crate::network::NonDriver,
    rx_message: Receiver<HotstuffMessage>,
    delegate: Arc<Box<dyn RPathDelegate>>,
    order_block_mgr: OrderBlockMgr,
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
        delegate: Arc<Box<dyn RPathDelegate>>,
    ) {
        let last_committed_round = store.header_block().map_or(0, |block| block.round());
        let mut round = 0;
        let mut high_qc = HotstuffBlockQC::default();

        for block in store.prepares().values() {
            if block.round() > round {
                round = block.round();
            }

            if let Some(qc) = block.qc().as_ref() {
                if qc.round > high_qc.round {
                    high_qc = qc.clone();
                }
            }
        }

        for block in store.pre_commits().values() {
            if block.round() > round {
                round = block.round();
            }

            if let Some(qc) = block.qc().as_ref() {
                if qc.round > high_qc.round {
                    high_qc = qc.clone();
                }
            }
        }

        let obj = Self {
            local_id,
            committee,
            store,
            signer,
            round,
            last_committed_round,
            high_qc,
            timer: Timer::new(store.group().consensus_interval()),
            vote_mgr,
            network_sender,
            rx_message,
            delegate,
            order_block_mgr: OrderBlockMgr::new(),
            non_driver,
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
        {
            if let Some(last_proposal) = block.proposals().last() {
                if &last_proposal.result_state != block.result_state_id() {
                    log::warn!("the result-state({}) in last-proposal is missmatch with block.result_state_id({})"
                        , last_proposal.result_state, block.result_state_id());
                    return Err(BuckyError::new(
                        BuckyErrorCode::Unmatch,
                        "result-state unmatch",
                    ));
                }
            }
        }

        {
            // check leader
            let leader = self.committee.get_leader(block.group_chunk_id()).await?;
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
                    if block.proposals().is_empty() {
                        match prev_block.as_ref() {
                            Some(prev_block) => {
                                if block.result_state_id() != prev_block.result_state_id() {
                                    log::warn!("block.result_state_id({}) is missmatch with prev_block.result_state_id({}) with no proposal."
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
                    (prev_block, proposals)
                }
                crate::storage::BlockLinkState::Pending => {
                    // 乱序，同步
                    return self.order_block_mgr.push_block(block).await;
                }
            }
        };

        {
            self.committee.verify_block(block).await?;
        }

        {
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
                    log::warn!("block verify failed by app, proposal: {}, prev_state: {}, expect-result: {}",
                        proposal_exe_info.proposal, prev_state_id, proposal_exe_info.result_state);
                }
            }

            assert_eq!(
                &prev_state_id,
                block.result_state_id(),
                "the result state is missmatched"
            );
        }

        unimplemented!()
    }

    async fn process_block(&mut self, block: &GroupConsensusBlock) -> BuckyResult<()> {
        /**
         * 验证过的块执行这个函数
         */
        unimplemented!()
    }

    async fn handle_vote(&mut self, vote: &HotstuffBlockQCVote) -> BuckyResult<()> {
        unimplemented!()
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
