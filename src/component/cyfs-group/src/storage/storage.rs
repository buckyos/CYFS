use std::collections::HashMap;

use cyfs_base::{BuckyResult, Group, ObjectId};
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal, GroupRPath};

pub enum BlockLinkState {
    Expired,
    DuplicateProposal,
    Duplicate,
    Link(
        Option<GroupConsensusBlock>,
        HashMap<ObjectId, GroupProposal>,
    ), // <prev-block, proposals>
    Pending,
}

pub struct Storage {
    group: Group,
    group_id: ObjectId,
    dec_id: ObjectId,
    rpath: String,

    dec_state_id: Option<ObjectId>,
    group_chunk_id: ObjectId,
    height: u64,
    last_vote_round: u64, // 参与投票的最后一个轮次
    header_block: Option<GroupConsensusBlock>,
    first_block: Option<GroupConsensusBlock>,
    prepares: HashMap<ObjectId, GroupConsensusBlock>,
    pre_commits: HashMap<ObjectId, GroupConsensusBlock>,
}

impl Storage {
    pub async fn load(group_id: &ObjectId, dec_id: &ObjectId, rpath: &str) -> BuckyResult<Storage> {
        // 用hash加载chunk
        // 从chunk解析group
        unimplemented!()
    }

    pub async fn insert(
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
    ) -> BuckyResult<Storage> {
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

    pub fn prepare_height(&self) -> u64 {
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

    pub fn get_rpath_by_id(rpath_id: &ObjectId) -> BuckyResult<Option<GroupRPath>> {
        unimplemented!()
    }

    pub async fn push_block(
        &mut self,
        block: GroupConsensusBlock,
        proposals: &[&GroupProposal],
    ) -> BuckyResult<()> {
        /**
         * 1. 把block存入prepares
         * 2. 把block.qc.block从prepares存入pre-commits
         * 3. 把block.qc.block.qc.block从pre-commits存入链上
         * 4. 把淘汰block清理掉
         */
        unimplemented!()
    }

    pub async fn last_vote_round(&self) -> u64 {
        unimplemented!()
    }

    pub async fn set_last_vote_round(&mut self, round: u64) -> BuckyResult<()> {
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
}
