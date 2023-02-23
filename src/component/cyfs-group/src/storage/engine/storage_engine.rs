use std::collections::{HashMap, HashSet};

use cyfs_base::{BuckyResult, ObjectId};
use cyfs_core::{GroupConsensusBlock, HotstuffBlockQC, HotstuffTimeout};

pub struct FinishProposalMgr {
    pub flip_timestamp: u64,
    pub over: HashSet<ObjectId>,
    pub adding: HashSet<ObjectId>,
}

pub struct StorageCacheInfo {
    pub dec_state_id: Option<ObjectId>, // commited/header state id
    pub last_vote_round: u64,           // 参与投票的最后一个轮次
    pub last_qc: Option<HotstuffBlockQC>,
    pub last_tc: Option<HotstuffTimeout>,
    pub header_block: Option<GroupConsensusBlock>,
    pub first_block: Option<GroupConsensusBlock>,
    pub prepares: HashMap<ObjectId, GroupConsensusBlock>,
    pub pre_commits: HashMap<ObjectId, GroupConsensusBlock>,
    pub finish_proposals: FinishProposalMgr,
}

impl StorageCacheInfo {
    pub fn new(dec_state_id: Option<ObjectId>) -> Self {
        Self {
            dec_state_id,
            last_vote_round: 0,
            last_qc: None,
            last_tc: None,
            header_block: None,
            first_block: None,
            prepares: HashMap::new(),
            pre_commits: HashMap::new(),
            finish_proposals: FinishProposalMgr {
                flip_timestamp: 0,
                over: HashSet::new(),
                adding: HashSet::new(),
            },
        }
    }
}

#[async_trait::async_trait]
pub trait StorageWriter: Send + Sync {
    async fn insert_prepares(&mut self, block_id: &ObjectId) -> BuckyResult<()>;
    async fn insert_pre_commit(&mut self, block_id: &ObjectId, is_instead: bool)
        -> BuckyResult<()>;
    async fn push_commit(
        &mut self,
        height: u64,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
        prev_result_state_id: &Option<ObjectId>,
        min_height: u64,
    ) -> BuckyResult<()>;
    async fn remove_prepares(&mut self, block_ids: &[ObjectId]) -> BuckyResult<()>;
    async fn push_proposals(
        &mut self,
        proposal_ids: &[ObjectId],
        timestamp: Option<(u64, u64)>, // (timestamp, prev_timestamp), 0 if the first
    ) -> BuckyResult<()>;

    async fn set_last_vote_round(&mut self, round: u64, prev_value: u64) -> BuckyResult<()>;
    async fn save_last_qc(&mut self, qc_id: &ObjectId) -> BuckyResult<()>;
    async fn save_last_tc(&mut self, tc_id: &ObjectId) -> BuckyResult<()>;

    async fn commit(mut self) -> BuckyResult<()>;
}

#[async_trait::async_trait]
pub trait StorageEngine {
    async fn find_block_by_height(&self, height: u64) -> BuckyResult<ObjectId>;
    // async fn is_proposal_finished(&self, proposal_id: &ObjectId) -> BuckyResult<bool>;
}
