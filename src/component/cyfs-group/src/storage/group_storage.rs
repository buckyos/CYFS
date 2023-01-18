use std::collections::HashMap;

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, Group, NamedObject, ObjectDesc, ObjectId,
};
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal};

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
    last_vote_round: u64, // 参与投票的最后一个轮次
    header_block: Option<GroupConsensusBlock>,
    first_block: Option<GroupConsensusBlock>,
    prepares: HashMap<ObjectId, GroupConsensusBlock>,
    pre_commits: HashMap<ObjectId, GroupConsensusBlock>,
}

impl GroupStorage {
    pub async fn create(
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
        init_state_id: Option<ObjectId>,
    ) -> BuckyResult<GroupStorage> {
        // 用hash加载chunk
        // 从chunk解析group
        unimplemented!()
    }

    pub async fn load(
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
    ) -> BuckyResult<GroupStorage> {
        // 用hash加载chunk
        // 从chunk解析group
        unimplemented!()
    }

    pub fn header_block(&self) -> &Option<GroupConsensusBlock> {
        &self.header_block
    }

    pub fn header_round(&self) -> u64 {
        self.header_block.as_ref().map_or(0, |b| b.round())
    }

    pub fn header_height(&self) -> u64 {
        self.header_block.as_ref().map_or(0, |b| b.height())
    }

    pub fn first_block(&self) -> &Option<GroupConsensusBlock> {
        &self.first_block
    }

    pub fn prepares(&self) -> &HashMap<ObjectId, GroupConsensusBlock> {
        &self.prepares
    }

    pub fn pre_commits(&self) -> &HashMap<ObjectId, GroupConsensusBlock> {
        &self.pre_commits
    }

    pub fn group(&self) -> &Group {
        &self.group
    }

    pub fn group_chunk_id(&self) -> &ObjectId {
        &self.group_chunk_id
    }

    pub fn dec_state_id(&self) -> &Option<ObjectId> {
        &self.dec_state_id
    }

    pub async fn get_block_by_height(&self, height: u64) -> BuckyResult<GroupConsensusBlock> {
        let header_height = self.header_height();
        let block = match height.cmp(&header_height) {
            std::cmp::Ordering::Less => {
                if height == self.first_block.as_ref().map_or(0, |b| b.height()) {
                    self.first_block.clone()
                } else {
                    // TODO find in storage
                    unimplemented!()
                }
            }
            std::cmp::Ordering::Equal => self.header_block.clone(),
            std::cmp::Ordering::Greater => {
                if height == header_height + 1 {
                    self.pre_commits
                        .iter()
                        .find(|(_, block)| block.height() == height)
                        .or(self
                            .prepares
                            .iter()
                            .find(|(_, block)| block.height() == height))
                        .map(|(_, block)| block.clone())
                } else if height == header_height + 2 {
                    self.prepares
                        .iter()
                        .find(|(_, block)| block.height() == height)
                        .map(|(_, block)| block.clone())
                } else {
                    None
                }
            }
        };

        block.ok_or(BuckyError::new(BuckyErrorCode::NotFound, "not found"))
    }

    pub async fn push_block(
        &mut self,
        block: GroupConsensusBlock,
    ) -> BuckyResult<Option<(&GroupConsensusBlock, Vec<GroupConsensusBlock>)>> {
        let header_height = self.header_height();
        assert!(block.height() > header_height && block.height() <= header_height + 3);

        let block_id = block.named_object().desc().object_id();
        let prev_block_id = block.prev_block_id();

        let mut remove_prepares = vec![];
        let mut new_pre_commit = None;
        let mut new_header = None;

        // prepare update memory
        if let Some(prev_block_id) = prev_block_id {
            if let Some(prev_block) = self.prepares.get(prev_block_id) {
                new_pre_commit = Some((prev_block_id.clone(), prev_block.clone()));

                if let Some(prev_prev_block_id) = prev_block.prev_block_id() {
                    if let Some(prev_prev_block) = self.pre_commits.get(prev_prev_block_id) {
                        assert_eq!(block.height(), header_height + 3);
                        assert_eq!(prev_prev_block.height(), header_height + 1);

                        new_header = Some(prev_prev_block.clone());
                        let new_header_id = prev_prev_block.named_object().desc().object_id();

                        for (id, block) in self.prepares.iter() {
                            if block.prev_block_id().map(|prev_id| {
                                prev_id == &new_header_id || prev_id == prev_block_id
                            }) != Some(true)
                            {
                                remove_prepares.push(id.clone());
                            }
                        }
                    } else {
                        assert_eq!(block.height(), header_height + 2);
                    }
                }
            } else {
                assert_eq!(block.height(), header_height + 1);
            }
        }

        /**
         * 1. 把block存入prepares
         * 2. 把block.qc.block从prepares存入pre-commits
         * 3. 把block.qc.block.qc.block从pre-commits存入链上
         * 4. 把其他分叉block清理掉
         * 5. 追加去重proposal, 注意翻页清理过期proposal
         * 6. 如果header有变更，返回新的header和被清理的分叉blocks
         */
        // TODO: storage
        // unimplemented!()

        // update memory
        if self.prepares.insert(block_id, block).is_some() {
            assert!(false);
        }

        match new_header {
            Some(new_header) => {
                self.dec_state_id = new_header.result_state_id().clone();
                self.header_block = Some(new_header);
                let mut removed_blocks =
                    HashMap::from([new_pre_commit.expect("shoud got new pre-commit block")]);

                std::mem::swap(&mut self.pre_commits, &mut removed_blocks);
                let mut removed_blocks: Vec<GroupConsensusBlock> =
                    removed_blocks.into_values().collect();

                for id in remove_prepares.iter() {
                    removed_blocks.push(self.prepares.remove(id).unwrap());
                }

                if self.first_block.is_none() {
                    self.first_block = self.header_block.clone();
                }
                return Ok(Some((self.header_block.as_ref().unwrap(), removed_blocks)));
            }
            None => {
                if let Some(new_pre_commit) = new_pre_commit {
                    if self
                        .pre_commits
                        .insert(new_pre_commit.0, new_pre_commit.1)
                        .is_some()
                    {
                        assert!(false);
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn last_vote_round(&self) -> u64 {
        self.last_vote_round
    }

    pub async fn set_last_vote_round(&mut self, round: u64) -> BuckyResult<()> {
        if round <= self.last_vote_round {
            return Ok(());
        }

        // TODO: storage
        unimplemented!();

        self.last_vote_round = round;

        Ok(())
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

    pub fn find_block_in_cache(&self, block_id: &ObjectId) -> BuckyResult<GroupConsensusBlock> {
        if let Some(block) = self.header_block.as_ref() {
            if &block.named_object().desc().object_id() == block_id {
                return Ok(block.clone());
            }
        }

        self.prepares
            .get(block_id)
            .or(self.pre_commits.get(block_id))
            .ok_or(BuckyError::new(BuckyErrorCode::NotFound, "not found"))
            .map(|block| block.clone())
    }

    pub fn find_block_in_cache_by_round(&self, round: u64) -> BuckyResult<GroupConsensusBlock> {
        let header_round = self.header_round();

        let found = match round.cmp(&header_round) {
            std::cmp::Ordering::Less => {
                return Err(BuckyError::new(BuckyErrorCode::NotFound, "not found"))
            }
            std::cmp::Ordering::Equal => self.header_block.as_ref(),
            std::cmp::Ordering::Greater => if round == header_round + 1 {
                self.pre_commits
                    .iter()
                    .find(|(_, block)| block.round() == round)
                    .or(self
                        .prepares
                        .iter()
                        .find(|(_, block)| block.round() == round))
            } else {
                self.prepares
                    .iter()
                    .find(|(_, block)| block.round() == round)
                    .or(self
                        .pre_commits
                        .iter()
                        .find(|(_, block)| block.round() == round))
            }
            .map(|(_, block)| block),
        };

        found
            .ok_or(BuckyError::new(BuckyErrorCode::NotFound, "not found"))
            .map(|block| block.clone())
    }

    pub async fn is_proposal_finished(
        &self,
        proposal_id: &ObjectId,
        prev_block_id: &ObjectId,
    ) -> BuckyResult<bool> {
        let prev_block = self.find_block_in_cache(prev_block_id);

        // find in cache
        if let Ok(prev_block) = prev_block.as_ref() {
            match prev_block
                .proposals()
                .iter()
                .find(|proposal| &proposal.proposal == proposal_id)
            {
                Some(_) => return Ok(true),
                None => {
                    if let Some(prev_prev_block_id) = prev_block.prev_block_id() {
                        let prev_prev_block = self.find_block_in_cache(prev_prev_block_id);
                        if let Ok(prev_prev_block) = prev_prev_block.as_ref() {
                            if prev_prev_block
                                .proposals()
                                .iter()
                                .find(|proposal| &proposal.proposal == proposal_id)
                                .is_some()
                            {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }

        // TODO: find in storage

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
