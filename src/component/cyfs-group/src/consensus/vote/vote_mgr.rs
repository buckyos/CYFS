use std::collections::{HashMap, HashSet};

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, NamedObject, ObjectDesc, ObjectId, Signature,
};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, HotstuffBlockQC, HotstuffBlockQCSign,
    HotstuffTimeout, HotstuffTimeoutSign,
};

use crate::{Committee, HotstuffBlockQCVote, HotstuffTimeoutVote};

pub struct VoteMgr {
    committee: Committee,
    round: u64,
    blocks: HashMap<ObjectId, GroupConsensusBlock>,
    votes: HashMap<u64, HashMap<ObjectId, Box<QCMaker>>>, // <round, <block-id, QC>>
    timeouts: HashMap<u64, Box<TCMaker>>,                 // <round, TC>
}

pub enum VoteThresholded {
    QC(HotstuffBlockQC),
    TC(HotstuffTimeout),
    None,
}

impl VoteMgr {
    pub fn new(committee: Committee) -> Self {
        Self {
            committee,
            votes: HashMap::new(),
            timeouts: HashMap::new(),
            round: 0,
            blocks: HashMap::new(),
        }
    }

    pub async fn add_voting_block(
        &mut self,
        block: &GroupConsensusBlock,
    ) -> BuckyResult<VoteThresholded> {
        if block.round() < self.round {
            return Ok(None);
        }

        let block_id = block.named_object().desc().object_id();
        self.blocks.insert(block_id, block.clone());

        if let Some(votes) = self.votes.get(&block.round()) {
            if let Some(qc_maker) = votes.get(&block_id) {
                if let Some(qc) = qc_maker.on_block(block, &self.committee).await? {
                    return Ok(VoteThresholded::QC(qc));
                }
            }
        }

        // TODO timeout要取high_qc的最大值block
        if let Some(votes) = self.timeouts.get(&block.round()) {
            if let Some(qc_maker) = votes.get(&block_id) {
                if let Some(qc) = qc_maker.on_block(block, &self.committee).await? {
                    return Ok(VoteThresholded::QC(qc));
                }
            }
        }
    }

    pub async fn add_vote(
        &mut self,
        vote: HotstuffBlockQCVote,
    ) -> BuckyResult<Option<HotstuffBlockQC>> {
        let block_id = vote.block_id;
        // Add the new vote to our aggregator and see if we have a QC.
        self.votes
            .entry(vote.round)
            .or_insert_with(HashMap::new)
            .entry(vote.block_id)
            .or_insert_with(|| Box::new(QCMaker::new()))
            .append(vote, &self.committee, self.blocks.get(&block_id))
            .await
    }

    pub async fn add_timeout(
        &mut self,
        timeout: HotstuffTimeoutVote,
    ) -> BuckyResult<Option<HotstuffTimeout>> {
        let block_id = timeout.high_qc.block_id;
        // Add the new timeout to our aggregator and see if we have a TC.
        self.timeouts
            .entry(timeout.round)
            .or_insert_with(|| Box::new(TCMaker::new(timeout.round)))
            .append(timeout, &self.committee, self.blocks.get(&block_id))
            .await
    }

    pub fn cleanup(&mut self, round: u64) {
        self.votes.retain(|k, _| k >= &round);
        self.timeouts.retain(|k, _| k >= &round);
        self.blocks.retain(|_, block| block.round() >= round);
        self.round = round;
    }
}

struct QCMaker {
    votes: Vec<(ObjectId, Signature)>,
    used: HashSet<ObjectId>,
    thresholded: bool,
}

impl QCMaker {
    pub fn new() -> Self {
        Self {
            votes: Vec::new(),
            used: HashSet::new(),
            thresholded: false,
        }
    }

    /// Try to append a signature to a (partial) quorum.
    pub async fn append(
        &mut self,
        vote: HotstuffBlockQCVote,
        committee: &Committee,
        block: Option<&GroupConsensusBlock>,
    ) -> BuckyResult<Option<HotstuffBlockQC>> {
        let author = vote.voter;

        if !self.used.insert(author) {
            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "has voted"));
        }

        self.votes.push((author, vote.signature));

        match block {
            Some(block) => self.on_block(block, committee).await,
            None => Ok(None),
        }
    }

    pub async fn on_block(
        &mut self,
        block: &GroupConsensusBlock,
        committee: &Committee,
    ) -> BuckyResult<Option<HotstuffBlockQC>> {
        if !self.thresholded {
            self.thresholded = committee
                .quorum_threshold(
                    &self.votes.iter().map(|v| v.0).collect(),
                    block.group_chunk_id(),
                )
                .await?;
            if self.thresholded {
                return Ok(Some(HotstuffBlockQC {
                    block_id: block.named_object().desc().object_id(),
                    round: block.round(),
                    votes: self
                        .votes
                        .iter()
                        .map(|(voter, signature)| HotstuffBlockQCSign {
                            voter: voter.clone(),
                            signature: signature.clone(),
                        })
                        .collect(),
                }));
            }
        }
        Ok(None)
    }
}

struct TCMaker {
    round: u64,
    votes: Vec<(ObjectId, Signature, u64)>,
    used: HashSet<ObjectId>,
    thresholded: bool,
}

impl TCMaker {
    pub fn new(round: u64) -> Self {
        Self {
            round,
            votes: Vec::new(),
            used: HashSet::new(),
            thresholded: false,
        }
    }

    /// Try to append a signature to a (partial) quorum.
    pub async fn append(
        &mut self,
        timeout: HotstuffTimeoutVote,
        committee: &Committee,
        block: Option<&GroupConsensusBlock>,
    ) -> BuckyResult<Option<HotstuffTimeout>> {
        let author = timeout.voter;

        assert_eq!(self.round, timeout.round);

        // Ensure it is the first time this authority votes.
        if !self.used.insert(author) {
            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "has voted"));
        }

        // Add the timeout to the accumulator.
        self.votes
            .push((author, timeout.signature, timeout.high_qc.round));

        match block {
            Some(block) => self.on_block(block, committee).await,
            None => Ok(None),
        }
    }

    pub async fn on_block(
        &mut self,
        block: &GroupConsensusBlock,
        committee: &Committee,
    ) -> BuckyResult<Option<HotstuffTimeout>> {
        if !self.thresholded {
            self.thresholded = committee
                .quorum_threshold(
                    &self.votes.iter().map(|v| v.0).collect(),
                    block.group_chunk_id(),
                )
                .await?;
            if self.thresholded {
                return Ok(Some(HotstuffTimeout {
                    round: self.round,
                    votes: self
                        .votes
                        .iter()
                        .map(|v| HotstuffTimeoutSign {
                            voter: v.0,
                            high_qc_round: v.2,
                            signature: v.1.clone(),
                        })
                        .collect(),
                }));
            }
        }
        Ok(None)
    }
}
