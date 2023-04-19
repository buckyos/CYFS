use std::collections::{HashMap, HashSet};

use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId, Signature};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, HotstuffBlockQC, HotstuffBlockQCSign,
    HotstuffTimeout, HotstuffTimeoutSign,
};
use cyfs_group_lib::{HotstuffBlockQCVote, HotstuffTimeoutVote};

use crate::Committee;

pub(crate) struct VoteMgr {
    committee: Committee,
    round: u64,
    blocks: HashMap<ObjectId, GroupConsensusBlock>,
    votes: HashMap<u64, HashMap<ObjectId, Box<QCMaker>>>, // <round, <block-id, QC>>
    timeouts: HashMap<u64, HashMap<ObjectId, Box<TCMaker>>>, // <round, <group-shell-id, TC>>
}

pub(crate) enum VoteThresholded {
    QC(HotstuffBlockQC),
    None,
}

// TODO: 丢弃太大的round，避免恶意节点恶意用太大的round攻击内存

impl VoteMgr {
    pub fn new(committee: Committee, round: u64) -> Self {
        Self {
            committee,
            votes: HashMap::new(),
            timeouts: HashMap::new(),
            round,
            blocks: HashMap::new(),
        }
    }

    pub async fn add_voting_block(&mut self, block: &GroupConsensusBlock) -> VoteThresholded {
        if block.round() < self.round {
            return VoteThresholded::None;
        }

        let block_id = block.block_id().object_id();
        self.blocks.insert(block_id.clone(), block.clone());

        if let Some(qc_makers) = self.votes.get_mut(&block.round()) {
            if let Some(qc_maker) = qc_makers.get_mut(block_id) {
                if let Some(qc) = qc_maker
                    .on_block(block, &self.committee)
                    .await
                    .unwrap_or(None)
                {
                    return VoteThresholded::QC(qc);
                }
            }
        }

        VoteThresholded::None
    }

    pub(crate) async fn add_vote(
        &mut self,
        vote: HotstuffBlockQCVote,
        block: Option<GroupConsensusBlock>,
    ) -> BuckyResult<Option<(HotstuffBlockQC, GroupConsensusBlock)>> {
        assert!(block
            .as_ref()
            .map_or(true, |b| b.block_id().object_id() == &vote.block_id));

        let block_id = vote.block_id;

        if let Some(block) = block.as_ref() {
            self.blocks.insert(block_id, block.clone());
        }
        let block_ref = block.as_ref().or(self.blocks.get(&block_id));

        // Add the new vote to our aggregator and see if we have a QC.
        self.votes
            .entry(vote.round)
            .or_insert_with(HashMap::new)
            .entry(block_id)
            .or_insert_with(|| Box::new(QCMaker::new()))
            .append(vote, &self.committee, block_ref)
            .await
            .map(|vote| vote.map(|v| (v, block.unwrap().clone())))
    }

    pub(crate) async fn add_timeout(
        &mut self,
        timeout: HotstuffTimeoutVote,
    ) -> BuckyResult<Option<HotstuffTimeout>> {
        // Add the new timeout to our aggregator and see if we have a TC.
        let tc_maker = self
            .timeouts
            .entry(timeout.round)
            .or_insert_with(|| HashMap::new())
            .entry(timeout.group_shell_id.clone())
            .or_insert_with(|| {
                Box::new(TCMaker::new(timeout.round, timeout.group_shell_id.clone()))
            });

        tc_maker.append(timeout, &self.committee).await
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
                    Some(block.group_shell_id()),
                )
                .await?;
            if self.thresholded {
                return Ok(Some(HotstuffBlockQC {
                    block_id: block.block_id().object_id().clone(),
                    prev_block_id: block.prev_block_id().map(|id| id.clone()),
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
    votes: Vec<HotstuffTimeoutVote>,
    group_shell_id: ObjectId,
    used: HashSet<ObjectId>,
    thresholded: bool,
}

impl TCMaker {
    pub fn new(round: u64, group_shell_id: ObjectId) -> Self {
        Self {
            round,
            votes: Vec::new(),
            used: HashSet::new(),
            thresholded: false,
            group_shell_id,
        }
    }

    /// Try to append a signature to a (partial) quorum.
    pub async fn append(
        &mut self,
        timeout: HotstuffTimeoutVote,
        committee: &Committee,
    ) -> BuckyResult<Option<HotstuffTimeout>> {
        let author = timeout.voter;

        assert_eq!(self.round, timeout.round);

        // Ensure it is the first time this authority votes.
        if !self.used.insert(author) {
            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "has voted"));
        }

        // Add the timeout to the accumulator.
        self.votes.push(timeout);

        self.on_block(committee).await
    }

    pub async fn on_block(
        &mut self,
        committee: &Committee,
    ) -> BuckyResult<Option<HotstuffTimeout>> {
        if !self.thresholded {
            self.thresholded = committee
                .quorum_threshold(
                    &self.votes.iter().map(|v| v.voter).collect(),
                    Some(&self.group_shell_id),
                )
                .await?;

            if self.thresholded {
                return Ok(Some(HotstuffTimeout {
                    round: self.round,
                    votes: self
                        .votes
                        .iter()
                        .map(|v| HotstuffTimeoutSign {
                            voter: v.voter,
                            high_qc_round: v.high_qc.as_ref().map_or(0, |qc| qc.round),
                            signature: v.signature.clone(),
                        })
                        .collect(),
                    group_shell_id: Some(self.group_shell_id.clone()),
                }));
            }
        }
        Ok(None)
    }
}
