use std::collections::{HashMap, HashSet};

use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId};

use super::{StorageEngine, StorageWriter};

struct StorageEngineMockFinishProposalMgr {
    flip_timestamp: u64,
    over: HashSet<ObjectId>,
    adding: HashSet<ObjectId>,
}

pub struct StorageEngineMock {
    last_vote_round: u64,

    result_state_id: Option<ObjectId>,
    block_height_range: (u64, u64),

    commit_blocks: HashMap<u64, ObjectId>,
    prepare_blocks: HashSet<ObjectId>,
    pre_commit_blocks: HashSet<ObjectId>,

    finish_proposals: StorageEngineMockFinishProposalMgr,
}

impl StorageEngineMock {
    pub fn new() -> Self {
        Self {
            last_vote_round: 0,
            block_height_range: (0, 0),
            commit_blocks: HashMap::new(),
            prepare_blocks: HashSet::new(),
            pre_commit_blocks: HashSet::new(),
            result_state_id: None,
            finish_proposals: StorageEngineMockFinishProposalMgr {
                flip_timestamp: 0,
                over: HashSet::new(),
                adding: HashSet::new(),
            },
        }
    }

    pub async fn create_writer(&mut self) -> BuckyResult<StorageEngineMockWriter> {
        Ok(StorageEngineMockWriter { engine: self })
    }
}

#[async_trait::async_trait]
impl StorageEngine for StorageEngineMock {
    async fn find_block_by_height(&self, height: u64) -> BuckyResult<ObjectId> {
        self.commit_blocks
            .get(&height)
            .map(|id| id.clone())
            .ok_or(BuckyError::new(BuckyErrorCode::NotFound, "not found"))
    }

    // async fn is_proposal_finished(&self, proposal_id: &ObjectId) -> BuckyResult<bool> {
    //     let is_finished = self
    //         .finish_proposals
    //         .adding
    //         .get(proposal_id)
    //         .or(self.finish_proposals.over.get(proposal_id))
    //         .is_some();
    //     Ok(is_finished)
    // }
}

pub struct StorageEngineMockWriter<'a> {
    engine: &'a mut StorageEngineMock,
}

#[async_trait::async_trait]
impl<'a> StorageWriter for StorageEngineMockWriter<'a> {
    async fn insert_prepares(&mut self, block_id: &ObjectId) -> BuckyResult<()> {
        if !self.engine.prepare_blocks.insert(block_id.clone()) {
            assert!(false);
            return Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "block prepare twice",
            ));
        }
        Ok(())
    }

    async fn insert_pre_commit(
        &mut self,
        block_id: &ObjectId,
        is_instead: bool,
    ) -> BuckyResult<()> {
        if !self.engine.prepare_blocks.remove(block_id) {
            assert!(false);
            return Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "block should be prepared before pre-commit",
            ));
        }

        if is_instead {
            self.engine.pre_commit_blocks = HashSet::from([block_id.clone()]);
        } else {
            if !self.engine.pre_commit_blocks.insert(block_id.clone()) {
                assert!(false);
                return Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "block pre-commit twice",
                ));
            }
        }

        Ok(())
    }

    async fn push_commit(
        &mut self,
        height: u64,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
        prev_result_state_id: &Option<ObjectId>,
        min_height: u64,
    ) -> BuckyResult<()> {
        assert!(height > min_height);
        assert_eq!(height, self.engine.block_height_range.1 + 1);
        assert_eq!(prev_result_state_id, &self.engine.result_state_id);

        if self
            .engine
            .commit_blocks
            .insert(height, block_id.clone())
            .is_some()
        {
            assert!(false);
            return Err(BuckyError::new(
                BuckyErrorCode::ErrorState,
                "block commit twice",
            ));
        }

        self.engine.block_height_range.1 = height;
        self.engine.result_state_id = result_state_id.clone();

        Ok(())
    }

    async fn remove_prepares(&mut self, block_ids: &[ObjectId]) -> BuckyResult<()> {
        for block_id in block_ids {
            if !self.engine.prepare_blocks.remove(block_id) {
                assert!(false);
                return Err(BuckyError::new(
                    BuckyErrorCode::ErrorState,
                    "try remove prepare not exists",
                ));
            }
        }
        Ok(())
    }

    async fn push_proposals(
        &mut self,
        proposal_ids: &[ObjectId],
        timestamp: Option<(u64, u64)>, // (timestamp, prev_timestamp), 0 if the first
    ) -> BuckyResult<()> {
        if let Some((timestamp, prev_timestamp)) = timestamp {
            let mut new_over = HashSet::new();
            std::mem::swap(&mut new_over, &mut self.engine.finish_proposals.adding);
            std::mem::swap(&mut new_over, &mut self.engine.finish_proposals.over);
            assert_eq!(prev_timestamp, self.engine.finish_proposals.flip_timestamp);
            self.engine.finish_proposals.flip_timestamp = timestamp;
        }

        for proposal_id in proposal_ids {
            if !self
                .engine
                .finish_proposals
                .adding
                .insert(proposal_id.clone())
            {
                assert!(false);
                return Err(BuckyError::new(
                    BuckyErrorCode::AlreadyExists,
                    "dup finish proposal",
                ));
            }
        }

        Ok(())
    }

    async fn set_last_vote_round(&mut self, round: u64, prev_value: u64) -> BuckyResult<()> {
        assert_eq!(self.engine.last_vote_round, prev_value);
        self.engine.last_vote_round = round;

        Ok(())
    }

    async fn save_last_qc(&mut self, qc_id: &ObjectId) -> BuckyResult<()> {
        Ok(())
    }

    async fn save_last_tc(&mut self, tc_id: &ObjectId) -> BuckyResult<()> {
        Ok(())
    }

    async fn commit(mut self) -> BuckyResult<()> {
        Ok(())
    }
}
