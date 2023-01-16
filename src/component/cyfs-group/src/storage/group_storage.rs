use std::collections::HashMap;

use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, Group, ObjectId};
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal, GroupRPath};

use crate::IsCreateRPath;

pub enum BlockLinkState {
    Expired,
    DuplicateProposal,
    Duplicate,
    Link(
        Option<GroupConsensusBlock>,
        HashMap<ObjectId, GroupProposal>,
    ), // <prev-block, proposals>
    Pending,
    InvalidBranch,
}

pub struct GroupStorage {
    group: Group,
    group_id: ObjectId,
    dec_id: ObjectId,
    rpath: String,

    dec_state_id: Option<ObjectId>, // commited/header state id
    group_chunk_id: ObjectId,
    height: u64,          // commited/header height
    last_vote_round: u64, // 参与投票的最后一个轮次
    header_block: Option<GroupConsensusBlock>,
    first_block: Option<GroupConsensusBlock>,
    prepares: HashMap<ObjectId, GroupConsensusBlock>,
    pre_commits: HashMap<ObjectId, GroupConsensusBlock>,
}

impl GroupStorage {
    pub async fn load(
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
        is_auto_create: &IsCreateRPath,
    ) -> BuckyResult<GroupStorage> {
        // 用hash加载chunk
        // 从chunk解析group
        unimplemented!()
    }

    pub async fn insert(
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
    ) -> BuckyResult<GroupStorage> {
        unimplemented!()
    }

    pub fn header_block(&self) -> &Option<GroupConsensusBlock> {
        unimplemented!()
    }

    pub fn header_round(&self) -> u64 {
        unimplemented!()
    }

    pub fn header_height(&self) -> u64 {
        unimplemented!()
    }

    pub fn first_block(&self) -> &Option<GroupConsensusBlock> {
        unimplemented!()
    }

    pub fn prepares(&self) -> &HashMap<ObjectId, GroupConsensusBlock> {
        unimplemented!()
    }

    pub fn pre_commits(&self) -> &HashMap<ObjectId, GroupConsensusBlock> {
        unimplemented!()
    }

    pub fn group(&self) -> &Group {
        &self.group
    }

    pub fn group_chunk_id(&self) -> &ObjectId {
        unimplemented!()
    }

    pub fn dec_state_id(&self) -> ObjectId {
        unimplemented!()
    }

    async fn user_nonce(&self, user_id: &ObjectId) -> BuckyResult<Option<u64>> {
        unimplemented!()
    }

    pub fn insert_rpath(rpath_obj: &GroupRPath) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn get_block_by_height(&self, height: u64) -> BuckyResult<GroupConsensusBlock> {
        unimplemented!()
    }

    pub async fn push_block(
        &mut self,
        block: GroupConsensusBlock,
    ) -> BuckyResult<Option<(&GroupConsensusBlock, Vec<GroupConsensusBlock>)>> {
        /**
         * 1. 把block存入prepares
         * 2. 把block.qc.block从prepares存入pre-commits
         * 3. 把block.qc.block.qc.block从pre-commits存入链上
         * 4. 把其他分叉block清理掉
         * 5. 追加去重proposal
         * 6. 如果header有变更，返回新的header和被清理的分叉blocks
         */
        unimplemented!()
    }

    pub fn last_vote_round(&self) -> u64 {
        unimplemented!()
    }

    pub async fn set_last_vote_round(&mut self, round: u64) -> BuckyResult<()> {
        if round <= self.last_vote_round {
            return Ok(());
        }

        unimplemented!()
    }

    pub async fn block_linked(&self, block: &GroupConsensusBlock) -> BuckyResult<BlockLinkState> {
        if block.height() <= self.header_height() {
            return Ok(BlockLinkState::Expired);
        }

        let linked_state = match block.height().cmp(&(self.header_height() + 3)) {
            std::cmp::Ordering::Less => {
                // 重复block，BlockLinkState::Duplicate
                BlockLinkState::Link(None, HashMap::default())
            }
            std::cmp::Ordering::Equal => BlockLinkState::Link(None, HashMap::default()),
            std::cmp::Ordering::Greater => BlockLinkState::Pending,
        };

        // BlockLinkState::Link状态也可能因为缺少前序成为BlockLinkState::Pending
        // 去重proposal，BlockLinkState::DuplicateProposal，去重只检查相同分叉链上的proposal，不同分叉上允许有相同proposal
        // 检查Proposal时间戳，早于去重proposal集合区间，或者晚于当前系统时间戳一定时间

        Ok(linked_state)
    }

    pub async fn find_block_in_cache(
        &self,
        block_id: &ObjectId,
    ) -> BuckyResult<GroupConsensusBlock> {
        unimplemented!()
    }

    pub async fn find_block_in_cache_by_round(
        &self,
        round: u64,
    ) -> BuckyResult<GroupConsensusBlock> {
        unimplemented!()
    }

    pub async fn is_proposal_finished(
        &self,
        proposal_id: &ObjectId,
        pre_block_id: &ObjectId,
    ) -> BuckyResult<bool> {
        unimplemented!()
    }

    pub fn block_with_max_round(&self) -> Option<GroupConsensusBlock> {
        let mut max_round = 0;
        let mut max_block = None;
        for block in self.prepares().values() {
            if block.round() > max_round {
                max_round = block.round();
                max_block = Some(block);
            }
        }

        for block in self.pre_commits().values() {
            if block.round() > max_round {
                max_round = block.round();
                max_block = Some(block);
            }
        }
        max_block.map(|block| block.clone())
    }

    // (found_block, cached_blocks)
    pub async fn find_block_by_round(
        &self,
        round: u64,
    ) -> (BuckyResult<GroupConsensusBlock>, Vec<GroupConsensusBlock>) {
        if self.header_block.is_none() {
            return (
                Err(BuckyError::new(BuckyErrorCode::NotFound, "not exist")),
                vec![],
            );
        }

        let mut blocks = vec![];
        let mut block = self.header_block.clone().unwrap();
        let mut min_height = 1;
        let mut min_round = 1;
        let mut max_height = block.height();
        let mut max_round = block.round();

        while min_height < max_height {
            blocks.push(block.clone());
            match block.round().cmp(&round) {
                std::cmp::Ordering::Equal => {
                    return (Ok(block), blocks);
                }
                std::cmp::Ordering::Less => {
                    min_round = block.round() + 1;
                    min_height = block.height() + 1;
                }
                std::cmp::Ordering::Greater => {
                    max_round = block.round() - 1;
                    max_height = block.height() - 1;
                }
            }

            let height = min_height
                + (round - min_round) * (max_height - min_height) / (max_round - min_round);

            block = match self.get_block_by_height(height).await {
                Ok(block) => block,
                Err(e) => return (Err(e), blocks),
            }
        }

        if block.round() == round {
            (Ok(block), blocks)
        } else {
            (
                Err(BuckyError::new(BuckyErrorCode::NotFound, "not exist")),
                blocks,
            )
        }
    }
}
