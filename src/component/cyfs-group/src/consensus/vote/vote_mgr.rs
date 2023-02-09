use std::collections::{HashMap, HashSet};

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, NamedObject, ObjectDesc, ObjectId, Signature,
};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, HotstuffBlockQC, HotstuffBlockQCSign,
    HotstuffTimeout, HotstuffTimeoutSign,
};

use crate::{Committee, HotstuffBlockQCVote, HotstuffTimeoutVote};

pub(crate) struct VoteMgr {
    committee: Committee,
    round: u64,
    blocks: HashMap<ObjectId, GroupConsensusBlock>,
    votes: HashMap<u64, HashMap<ObjectId, Box<QCMaker>>>, // <round, <block-id, QC>>
    timeouts: HashMap<u64, Box<TCMaker>>,                 // <round, TC>
    waiting_timeouts: HashMap<ObjectId, HashMap<u64, HashMap<ObjectId, HotstuffTimeoutVote>>>, // <block-id, <round, <voter-id, TC>>>
}

pub(crate) enum VoteThresholded {
    QC(HotstuffBlockQC),
    TC(HotstuffTimeout, Option<GroupConsensusBlock>),
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
            waiting_timeouts: HashMap::new(),
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

        let mut timeouts: Vec<(&u64, &mut Box<TCMaker>)> = self
            .timeouts
            .iter_mut()
            .filter(|(round, tc_maker)| {
                **round >= block.round()
                    && tc_maker
                        .max_block()
                        .as_ref()
                        .map_or(false, |max_block_id| block_id == max_block_id)
            })
            .collect();

        timeouts.sort_unstable_by(|l, r| r.0.cmp(l.0));

        for (round, tc_maker) in timeouts {
            if let Some(tc) = tc_maker
                .on_block(Some(block), &self.committee)
                .await
                .unwrap_or(None)
            {
                return VoteThresholded::TC(tc, Some(block.clone()));
            }
        }

        let waiting_timeouts = self.waiting_timeouts.remove(block_id);
        if let Some(waiting_timeouts) = waiting_timeouts {
            let mut waiting_timeouts: Vec<(u64, HashMap<ObjectId, HotstuffTimeoutVote>)> =
                waiting_timeouts.into_iter().collect();
            waiting_timeouts.sort_unstable_by(|l, r| r.0.cmp(&l.0));

            for (_, timeouts) in waiting_timeouts {
                for (_, timeout) in timeouts {
                    if self
                        .committee
                        .verify_timeout(&timeout, Some(block))
                        .await
                        .is_ok()
                    {
                        if let Some((tc, block)) = self
                            .add_timeout(timeout, Some(block))
                            .await
                            .map_or(None, |r| r)
                        {
                            return VoteThresholded::TC(tc, block);
                        }
                    }
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
        block: Option<&GroupConsensusBlock>,
    ) -> BuckyResult<Option<(HotstuffTimeout, Option<GroupConsensusBlock>)>> {
        assert!(
            block.map(|block| block.block_id().object_id().clone())
                == timeout.high_qc.as_ref().map(|qc| qc.block_id)
        );

        // Add the new timeout to our aggregator and see if we have a TC.
        let tc_maker = self
            .timeouts
            .entry(timeout.round)
            .or_insert_with(|| Box::new(TCMaker::new(timeout.round)));

        if let Some(qc) = timeout.high_qc.as_ref() {
            self.blocks
                .insert(qc.block_id, block.clone().unwrap().clone());
        }

        let max_block = tc_maker
            .max_block()
            .or(timeout.high_qc.as_ref().map(|qc| qc.block_id))
            .as_ref()
            .map(|max_block_id| self.blocks.get(max_block_id).unwrap());

        tc_maker
            .append(timeout, &self.committee, max_block)
            .await
            .map(|vote| vote.map(|v| (v, max_block.cloned())))
    }

    pub(crate) fn add_waiting_timeout(&mut self, timeout: HotstuffTimeoutVote) {
        let block_id = timeout
            .high_qc
            .as_ref()
            .expect("pre-block is empty")
            .block_id;

        self.waiting_timeouts
            .entry(block_id)
            .or_insert_with(HashMap::new)
            .entry(timeout.round)
            .or_insert_with(HashMap::new)
            .entry(timeout.voter)
            .or_insert(timeout);
    }

    pub fn cleanup(&mut self, round: u64) {
        self.votes.retain(|k, _| k >= &round);
        self.timeouts.retain(|k, _| k >= &round);
        self.blocks.retain(|_, block| block.round() >= round);
        self.waiting_timeouts.retain(|_, timeouts| {
            timeouts.retain(|k, _| k >= &round);
            !timeouts.is_empty()
        });
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
                    Some(block.group_chunk_id()),
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
        self.votes.push(timeout);

        self.on_block(block, committee).await
    }

    pub async fn on_block(
        &mut self,
        block: Option<&GroupConsensusBlock>,
        committee: &Committee,
    ) -> BuckyResult<Option<HotstuffTimeout>> {
        if !self.thresholded {
            self.thresholded = committee
                .quorum_threshold(
                    &self.votes.iter().map(|v| v.voter).collect(),
                    block.map(|block| block.group_chunk_id()),
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
                }));
            }
        }
        Ok(None)
    }

    pub fn max_block(&self) -> Option<ObjectId> {
        self.votes
            .iter()
            .max_by(|l, r| {
                l.high_qc
                    .as_ref()
                    .map_or(0, |qc| qc.round)
                    .cmp(&r.high_qc.as_ref().map_or(0, |qc| qc.round))
            })
            .map_or(None, |v| v.high_qc.as_ref().map(|qc| qc.block_id))
    }
}
