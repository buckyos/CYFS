use std::collections::{HashMap, HashSet};

use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId, Signature};
use cyfs_core::{GroupConsensusBlock, HotstuffBlockQC, HotstuffTimeout, HotstuffTimeoutSign};

use crate::{Committee, HotstuffBlockQCVote, HotstuffTimeoutVote};

pub struct VoteMgr {
    committee: Committee,
    votes: HashMap<u64, HashMap<ObjectId, Box<QCMaker>>>,
    timeouts: HashMap<u64, Box<TCMaker>>,
}

impl VoteMgr {
    pub fn new(committee: Committee) -> Self {
        Self {
            committee,
            votes: HashMap::new(),
            timeouts: HashMap::new(),
        }
    }

    pub fn add_vote(&mut self, vote: HotstuffBlockQCVote) -> BuckyResult<Option<HotstuffBlockQC>> {
        // TODO [issue #7]: A bad node may make us run out of memory by sending many votes
        // with different round numbers or different digests.

        // Add the new vote to our aggregator and see if we have a QC.
        self.votes
            .entry(vote.round)
            .or_insert_with(HashMap::new)
            .entry(vote.block_id)
            .or_insert_with(|| Box::new(QCMaker::new()))
            .append(vote, &self.committee)
    }

    pub fn add_timeout(&mut self, timeout: HotstuffTimeoutVote) -> BuckyResult<Option<TC>> {
        // TODO: A bad node may make us run out of memory by sending many timeouts
        // with different round numbers.

        // Add the new timeout to our aggregator and see if we have a TC.
        self.timeouts
            .entry(timeout.round)
            .or_insert_with(|| Box::new(TCMaker::new()))
            .append(timeout, &self.committee)
    }

    pub fn cleanup(&mut self, round: &Round) {
        self.votes.retain(|k, _| k >= round);
        self.timeouts.retain(|k, _| k >= round);
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
    pub fn append(
        &mut self,
        vote: HotstuffBlockQCVote,
        committee: &Committee,
    ) -> BuckyResult<Option<HotstuffBlockQC>> {
        let author = vote.voter;

        if !self.used.insert(author) {
            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "has voted"));
        }

        self.votes.push((author, vote.signature));
        if !self.thresholded
            && committee
                .quorum_threshold(self.votes.iter().map(|v| v.0).collect(), &vote.block_id)?
        {
            self.thresholded = true;
            return Ok(Some(HotstuffBlockQC {
                block_id: vote.block_id,
                round: vote.round,
                votes: self.votes.clone(),
                dummy_round: todo!(),
            }));
        }
        Ok(None)
    }
}

struct TCMaker {
    votes: Vec<(ObjectId, Signature, u64)>,
    used: HashSet<ObjectId>,
    thresholded: bool,
}

impl TCMaker {
    pub fn new() -> Self {
        Self {
            votes: Vec::new(),
            used: HashSet::new(),
            thresholded: false,
        }
    }

    /// Try to append a signature to a (partial) quorum.
    pub fn append(
        &mut self,
        timeout: HotstuffTimeoutVote,
        committee: &Committee,
    ) -> BuckyResult<Option<HotstuffTimeout>> {
        let author = timeout.voter;

        // Ensure it is the first time this authority votes.
        if !self.used.insert(author) {
            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "has voted"));
        }

        // Add the timeout to the accumulator.
        self.votes
            .push((author, timeout.signature, timeout.high_qc.round));

        if !self.thresholded
            && committee.timeout_threshold(
                self.votes.iter().map(|v| v.0).collect(),
                &timeout.high_qc.block_id,
            )?
        {
            self.thresholded = true;
            return Ok(Some(HotstuffTimeout {
                round: timeout.round,
                votes: self.votes.iter().map(|v| HotstuffTimeoutSign {
                    voter: v.0,
                    high_qc_round: v.2,
                    signature: v.1,
                }),
            }));
        }
        Ok(None)
    }
}
