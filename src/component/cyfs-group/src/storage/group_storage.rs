use std::{
    collections::{HashMap, HashSet},
    time::SystemTime,
};

use cyfs_base::{
    bucky_time_to_system_time, BuckyError, BuckyErrorCode, BuckyResult, Group, NamedObject,
    ObjectDesc, ObjectId, ObjectMap, ObjectMapOpEnvMemoryCache, ObjectTypeCode, RawDecode,
};
use cyfs_chunk_lib::ChunkMeta;
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal, GroupQuorumCertificate,
    GroupQuorumCertificateObject, HotstuffBlockQC, HotstuffTimeout,
};
use cyfs_lib::GlobalStateManagerRawProcessorRef;

use crate::{
    storage::StorageWriter, GroupObjectMapProcessor, GroupRPathStatus, GroupStatePath,
    NONDriverHelper, PROPOSAL_MAX_TIMEOUT, STATE_PATH_SEPARATOR, TIME_PRECISION,
};

use super::{
    engine::{
        GroupObjectMapProcessorGroupState, StorageCacheInfo, StorageEngineGroupState,
        StorageEngineMock,
    },
    StorageEngine,
};

const PROPOSAL_MAX_TIMEOUT_AS_MICRO_SEC: u64 = PROPOSAL_MAX_TIMEOUT.as_micros() as u64;

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
    local_device_id: ObjectId,
    non_driver: NONDriverHelper,
    group_chunk_id: ObjectId,

    cache: StorageCacheInfo,

    storage_engine: StorageEngineGroupState,
    object_map_processor: GroupObjectMapProcessorGroupState,
}

impl GroupStorage {
    pub(crate) async fn create(
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
        non_driver: NONDriverHelper,
        local_device_id: ObjectId,
        root_state_mgr: &GlobalStateManagerRawProcessorRef,
    ) -> BuckyResult<GroupStorage> {
        let group = non_driver.get_group(group_id, None, None).await?;
        let group_chunk = ChunkMeta::from(&group);
        let group_chunk_id = group_chunk.to_chunk().await.unwrap().calculate_id();

        let group_state = root_state_mgr
            .load_root_state(group_id, Some(group_id.clone()), true)
            .await?
            .expect("create group state failed.");

        let dec_group_state = group_state.get_dec_root_manager(dec_id, true).await?;
        let object_map_processor = GroupObjectMapProcessorGroupState::new(&dec_group_state);

        Ok(Self {
            group,
            group_id: group_id.clone(),
            dec_id: dec_id.clone(),
            rpath: rpath.to_string(),
            non_driver,
            group_chunk_id: group_chunk_id.object_id(),
            storage_engine: StorageEngineGroupState::new(
                dec_group_state,
                GroupStatePath::new(rpath.to_string()),
            ),
            local_device_id,
            cache: StorageCacheInfo::new(None),
            object_map_processor,
        })
    }

    pub(crate) async fn load(
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
        non_driver: NONDriverHelper,
        local_device_id: ObjectId,
        root_state_mgr: &GlobalStateManagerRawProcessorRef,
    ) -> BuckyResult<GroupStorage> {
        // 用hash加载chunk
        // 从chunk解析group

        let group = non_driver
            .get_group(group_id, None, None)
            .await
            .map_err(|err| {
                log::warn!("get group {} from noc failed {:?}", group_id, err);
                err
            })?;
        let group_chunk = ChunkMeta::from(&group);
        let group_chunk_id = group_chunk.to_chunk().await.unwrap().calculate_id();

        let group_state = root_state_mgr
            .load_root_state(group_id, Some(group_id.clone()), true)
            .await
            .map_err(|err| {
                log::warn!("load root state for group {} failed {:?}", group_id, err);
                err
            })?
            .expect("create group state failed.");

        let dec_group_state = group_state
            .get_dec_root_manager(dec_id, true)
            .await
            .map_err(|err| {
                log::warn!("get root state manager for dec {} failed {:?}", dec_id, err);
                err
            })?;

        let state_path = GroupStatePath::new(rpath.to_string());
        let cache =
            StorageEngineGroupState::load_cache(&dec_group_state, &non_driver, &state_path).await?;
        let object_map_processor = GroupObjectMapProcessorGroupState::new(&dec_group_state);

        Ok(Self {
            group,
            group_id: group_id.clone(),
            dec_id: dec_id.clone(),
            rpath: rpath.to_string(),
            non_driver,
            group_chunk_id: group_chunk_id.object_id(),
            storage_engine: StorageEngineGroupState::new(dec_group_state, state_path),
            local_device_id,
            cache,
            object_map_processor,
        })
    }

    pub fn header_block(&self) -> &Option<GroupConsensusBlock> {
        &self.cache.header_block
    }

    pub fn header_round(&self) -> u64 {
        self.cache.header_block.as_ref().map_or(0, |b| b.round())
    }

    pub fn header_height(&self) -> u64 {
        self.cache.header_block.as_ref().map_or(0, |b| b.height())
    }

    pub fn first_block(&self) -> &Option<GroupConsensusBlock> {
        &self.cache.first_block
    }

    pub fn prepares(&self) -> &HashMap<ObjectId, GroupConsensusBlock> {
        &self.cache.prepares
    }

    pub fn pre_commits(&self) -> &HashMap<ObjectId, GroupConsensusBlock> {
        &self.cache.pre_commits
    }

    pub fn group(&self) -> &Group {
        &self.group
    }

    pub fn group_chunk_id(&self) -> &ObjectId {
        &self.group_chunk_id
    }

    pub fn dec_state_id(&self) -> &Option<ObjectId> {
        &self.cache.dec_state_id
    }

    pub async fn get_block_by_height(&self, height: u64) -> BuckyResult<GroupConsensusBlock> {
        let header_height = self.header_height();
        let block = match height.cmp(&header_height) {
            std::cmp::Ordering::Less => {
                if height == self.cache.first_block.as_ref().map_or(0, |b| b.height()) {
                    self.cache.first_block.clone()
                } else {
                    // find in storage
                    let block_id = self.storage_engine.find_block_by_height(height).await?;
                    Some(self.non_driver.get_block(&block_id, None).await?)
                }
            }
            std::cmp::Ordering::Equal => self.cache.header_block.clone(),
            std::cmp::Ordering::Greater => {
                if height == header_height + 1 {
                    self.cache
                        .pre_commits
                        .iter()
                        .find(|(_, block)| block.height() == height)
                        .or(self
                            .cache
                            .prepares
                            .iter()
                            .find(|(_, block)| block.height() == height))
                        .map(|(_, block)| block.clone())
                } else if height == header_height + 2 {
                    self.cache
                        .prepares
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

        let block_id = block.block_id();
        let prev_block_id = block.prev_block_id();

        let mut remove_prepares = vec![];
        let mut new_pre_commit = None;
        let mut new_header = None;

        // prepare update memory
        if let Some(prev_block_id) = prev_block_id {
            if let Some(prev_block) = self.cache.prepares.get(prev_block_id) {
                new_pre_commit = Some((prev_block_id.clone(), prev_block.clone()));

                if let Some(prev_prev_block_id) = prev_block.prev_block_id() {
                    if let Some(prev_prev_block) = self.cache.pre_commits.get(prev_prev_block_id) {
                        assert_eq!(block.height(), header_height + 3);
                        assert_eq!(prev_prev_block.height(), header_height + 1);
                        assert_eq!(
                            prev_prev_block.prev_block_id(),
                            self.cache
                                .header_block
                                .as_ref()
                                .map(|b| b.block_id().object_id().clone())
                                .as_ref()
                        );

                        new_header = Some(prev_prev_block.clone());
                        let new_header_id = prev_prev_block.block_id().object_id();

                        for (id, block) in self.cache.prepares.iter() {
                            if block.prev_block_id().map(|prev_id| {
                                assert_ne!(prev_id, prev_block_id);
                                prev_id == new_header_id
                            }) != Some(true)
                                && id != prev_block_id
                            {
                                remove_prepares.push(id.clone());
                            }
                        }
                    } else {
                        assert_eq!(block.height(), header_height + 2);
                    }
                }
            } else {
                assert_ne!(block.height(), header_height + 3);
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
        // storage
        let mut writer = self.storage_engine.create_writer().await?;
        writer.insert_prepares(block_id.object_id()).await?;
        if let Some((new_pre_commit, _)) = new_pre_commit.as_ref() {
            writer
                .insert_pre_commit(new_pre_commit, new_header.is_some())
                .await?;
        }
        if let Some(new_header) = new_header.as_ref() {
            writer
                .push_commit(
                    new_header.height(),
                    new_header.block_id().object_id(),
                    new_header.result_state_id(),
                    self.cache
                        .header_block
                        .as_ref()
                        .map_or(&None, |b| b.result_state_id()),
                    self.cache.first_block.as_ref().map_or(0, |b| b.height()),
                )
                .await?;

            writer.remove_prepares(remove_prepares.as_slice()).await?;

            if new_header.proposals().len() > 0 {
                let finish_proposals: Vec<ObjectId> = new_header
                    .proposals()
                    .iter()
                    .map(|p| p.proposal.clone())
                    .collect();

                let timestamp = new_header.named_object().desc().create_time();
                // log::debug!(
                //     "push proposals storage flip-time from {} to {}",
                //     self.cache.finish_proposals.flip_timestamp,
                //     timestamp
                // );
                if timestamp - self.cache.finish_proposals.flip_timestamp
                    > PROPOSAL_MAX_TIMEOUT_AS_MICRO_SEC
                {
                    writer
                        .push_proposals(
                            finish_proposals.as_slice(),
                            Some((timestamp, self.cache.finish_proposals.flip_timestamp)),
                        )
                        .await?;
                } else {
                    writer
                        .push_proposals(finish_proposals.as_slice(), None)
                        .await?;
                }
            }
        }

        writer.commit().await?;

        // update memory
        if self
            .cache
            .prepares
            .insert(block_id.object_id().clone(), block)
            .is_some()
        {
            assert!(false);
        }

        match new_header {
            Some(new_header) => {
                self.cache.dec_state_id = new_header.result_state_id().clone();

                let new_pre_commit = new_pre_commit.expect("shoud got new pre-commit block");
                self.cache.prepares.remove(&new_pre_commit.0);

                let mut removed_blocks = HashMap::from([new_pre_commit]);

                std::mem::swap(&mut self.cache.pre_commits, &mut removed_blocks);
                let mut removed_blocks: Vec<GroupConsensusBlock> =
                    removed_blocks.into_values().collect();

                for id in remove_prepares.iter() {
                    removed_blocks.push(self.cache.prepares.remove(id).unwrap());
                }

                if self.cache.first_block.is_none() {
                    self.cache.first_block = Some(new_header.clone());
                }

                if new_header.proposals().len() > 0 {
                    let timestamp = new_header.named_object().desc().create_time();

                    // log::debug!(
                    //     "push proposals flip-time from {} to {}",
                    //     self.cache.finish_proposals.flip_timestamp,
                    //     timestamp
                    // );

                    if timestamp - self.cache.finish_proposals.flip_timestamp
                        > PROPOSAL_MAX_TIMEOUT_AS_MICRO_SEC
                    {
                        let mut new_over = HashSet::new();
                        std::mem::swap(&mut new_over, &mut self.cache.finish_proposals.adding);
                        std::mem::swap(&mut new_over, &mut self.cache.finish_proposals.over);
                        self.cache.finish_proposals.flip_timestamp = timestamp;
                    }

                    for proposal in new_header.proposals() {
                        let is_new = self.cache.finish_proposals.adding.insert(proposal.proposal);
                        assert!(is_new);
                    }
                }

                self.cache.header_block = Some(new_header);
                return Ok(Some((
                    self.cache.header_block.as_ref().unwrap(),
                    removed_blocks,
                )));
            }
            None => {
                if let Some(new_pre_commit) = new_pre_commit {
                    assert!(remove_prepares.is_empty());

                    if self
                        .cache
                        .pre_commits
                        .insert(new_pre_commit.0, new_pre_commit.1)
                        .is_some()
                    {
                        assert!(false);
                    }
                    self.cache
                        .prepares
                        .remove(&new_pre_commit.0)
                        .expect("any block in pre-commit should be from prepare");
                }
            }
        }

        Ok(None)
    }

    pub fn last_vote_round(&self) -> u64 {
        self.cache.last_vote_round
    }

    pub async fn set_last_vote_round(&mut self, round: u64) -> BuckyResult<()> {
        if round <= self.cache.last_vote_round {
            return Ok(());
        }

        // storage
        let mut writer = self.storage_engine.create_writer().await?;
        writer
            .set_last_vote_round(round, self.cache.last_vote_round)
            .await?;
        writer.commit().await?;

        self.cache.last_vote_round = round;

        Ok(())
    }

    pub fn last_qc(&self) -> &Option<HotstuffBlockQC> {
        &self.cache.last_qc
    }

    pub async fn save_qc(&mut self, qc: &HotstuffBlockQC) -> BuckyResult<()> {
        let quorum_round = qc.round;
        if quorum_round < self.cache.last_vote_round
            || quorum_round <= self.cache.last_qc.as_ref().map_or(0, |qc| qc.round)
        {
            return Ok(());
        }

        let qc = GroupQuorumCertificate::from(qc.clone());
        self.non_driver.put_qc(&qc).await?;

        let mut writer = self.storage_engine.create_writer().await?;
        writer.save_last_qc(&qc.desc().object_id()).await?;
        writer.commit().await?;

        self.cache.last_qc = Some(qc.try_into().unwrap());
        Ok(())
    }

    pub fn last_tc(&self) -> &Option<HotstuffTimeout> {
        &self.cache.last_tc
    }

    pub async fn save_tc(&mut self, tc: &HotstuffTimeout) -> BuckyResult<()> {
        let quorum_round = tc.round;
        if quorum_round < self.cache.last_vote_round
            || quorum_round <= self.cache.last_tc.as_ref().map_or(0, |tc| tc.round)
        {
            return Ok(());
        }

        let tc = GroupQuorumCertificate::from(tc.clone());
        self.non_driver.put_qc(&tc).await?;

        let mut writer = self.storage_engine.create_writer().await?;
        writer.save_last_tc(&tc.desc().object_id()).await?;
        writer.commit().await?;

        self.cache.last_tc = Some(tc.try_into().unwrap());
        Ok(())
    }

    pub async fn block_linked(&self, block: &GroupConsensusBlock) -> BuckyResult<BlockLinkState> {
        log::debug!(
            "[group storage] {} block_linked {} step1",
            self.local_device_id,
            block.block_id()
        );

        let header_height = self.header_height();
        if block.height() <= header_height {
            return Ok(BlockLinkState::Expired);
        }

        if block.height() > header_height + 3 {
            return Ok(BlockLinkState::Pending);
        };

        // BlockLinkState::Link状态也可能因为缺少前序成为BlockLinkState::Pending
        // 去重proposal，BlockLinkState::DuplicateProposal，去重只检查相同分叉链上的proposal，不同分叉上允许有相同proposal
        // 检查Proposal时间戳，早于去重proposal集合区间，或者晚于当前系统时间戳一定时间

        let block_id = block.block_id();

        if self.find_block_in_cache(block_id.object_id()).is_ok() {
            return Ok(BlockLinkState::Duplicate);
        }

        let now = SystemTime::now();
        let block_time = bucky_time_to_system_time(block.named_object().desc().create_time());

        if let Ok(duration) = block_time.duration_since(now) {
            if duration > TIME_PRECISION {
                return Err(BuckyError::new(
                    BuckyErrorCode::ErrorTimestamp,
                    "error timestamp",
                ));
            }
        }

        log::debug!(
            "[group storage] {} block_linked {} step2",
            self.local_device_id,
            block.block_id()
        );

        let prev_block = match block.prev_block_id() {
            Some(prev_block_id) => match self.find_block_in_cache(prev_block_id) {
                Ok(prev_block) => {
                    if prev_block.height() + 1 != block.height() {
                        return Err(BuckyError::new(BuckyErrorCode::Failed, "height error"));
                    } else if prev_block.round() >= block.round() {
                        return Err(BuckyError::new(BuckyErrorCode::Failed, "round error"));
                    } else {
                        let prev_block_time = bucky_time_to_system_time(
                            prev_block.named_object().desc().create_time(),
                        );
                        if let Ok(duration) = prev_block_time.duration_since(block_time) {
                            if duration > TIME_PRECISION {
                                return Err(BuckyError::new(
                                    BuckyErrorCode::ErrorTimestamp,
                                    "error timestamp",
                                ));
                            }
                        }
                        Some(prev_block)
                    }
                }
                Err(_) => {
                    if block.height() == header_height + 1 {
                        return Ok(BlockLinkState::InvalidBranch);
                    }
                    return Ok(BlockLinkState::Pending);
                }
            },
            None => {
                if block.height() != 1 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, "height error"));
                } else if header_height != 0 {
                    return Ok(BlockLinkState::InvalidBranch);
                } else {
                    None
                }
            }
        };

        log::debug!(
            "[group storage] {} block_linked {} step3",
            self.local_device_id,
            block.block_id()
        );

        let mut proposals = HashMap::new();
        let check_proposal_results =
            futures::future::join_all(block.proposals().iter().map(|proposal_result| async {
                if let Some(prev_block_id) = block.prev_block_id() {
                    if self
                        .is_proposal_finished(&proposal_result.proposal, prev_block_id)
                        .await?
                    {
                        return Ok(BlockLinkState::DuplicateProposal);
                    }
                }

                let proposal = self
                    .non_driver
                    .get_proposal(&proposal_result.proposal, Some(block.owner()))
                    .await?;

                let proposal_time = bucky_time_to_system_time(proposal.desc().create_time());
                if block_time
                    .duration_since(proposal_time)
                    .or(proposal_time.duration_since(block_time))
                    .unwrap()
                    > TIME_PRECISION
                {
                    return Err(BuckyError::new(
                        BuckyErrorCode::ErrorTimestamp,
                        "error timestamp",
                    ));
                }

                Ok(BlockLinkState::Link(
                    None,
                    HashMap::from([(proposal_result.proposal, proposal)]),
                ))
            }))
            .await;

        for check_result in check_proposal_results {
            match check_result {
                Ok(result) => match result {
                    BlockLinkState::Link(_, proposal) => {
                        let (proposal_id, proposal) = proposal.into_iter().next().unwrap();
                        if proposals.get(&proposal_id).is_some() {
                            return Ok(BlockLinkState::DuplicateProposal);
                        }
                        proposals.insert(proposal_id, proposal);
                    }
                    _ => return Ok(result),
                },
                _ => return check_result,
            }
        }

        log::debug!(
            "[group storage] {} block_linked {} step4",
            self.local_device_id,
            block.block_id()
        );

        Ok(BlockLinkState::Link(prev_block, proposals))
    }

    pub fn find_block_in_cache(&self, block_id: &ObjectId) -> BuckyResult<GroupConsensusBlock> {
        if let Some(block) = self.cache.header_block.as_ref() {
            if block.block_id().object_id() == block_id {
                return Ok(block.clone());
            }
        }

        self.cache
            .prepares
            .get(block_id)
            .or(self.cache.pre_commits.get(block_id))
            .ok_or(BuckyError::new(BuckyErrorCode::NotFound, "not found"))
            .map(|block| block.clone())
    }

    pub fn find_block_in_cache_by_round(&self, round: u64) -> BuckyResult<GroupConsensusBlock> {
        let header_round = self.header_round();

        let found = match round.cmp(&header_round) {
            std::cmp::Ordering::Less => {
                return Err(BuckyError::new(BuckyErrorCode::NotFound, "not found"))
            }
            std::cmp::Ordering::Equal => self.cache.header_block.as_ref(),
            std::cmp::Ordering::Greater => if round == header_round + 1 {
                self.cache
                    .pre_commits
                    .iter()
                    .find(|(_, block)| block.round() == round)
                    .or(self
                        .cache
                        .prepares
                        .iter()
                        .find(|(_, block)| block.round() == round))
            } else {
                self.cache
                    .prepares
                    .iter()
                    .find(|(_, block)| block.round() == round)
                    .or(self
                        .cache
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

        // find in storage

        let is_finished = self
            .cache
            .finish_proposals
            .adding
            .get(proposal_id)
            .or(self.cache.finish_proposals.over.get(proposal_id))
            .is_some();
        Ok(is_finished)
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
        if self.cache.header_block.is_none() {
            return (
                Err(BuckyError::new(BuckyErrorCode::NotFound, "not exist")),
                vec![],
            );
        }

        let mut blocks = vec![];
        let mut block = self.cache.header_block.clone().unwrap();
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

    pub async fn get_by_path(&self, sub_path: &str) -> BuckyResult<GroupRPathStatus> {
        let (header_block, qc) = match self.cache.header_block.as_ref() {
            Some(block) => {
                let (_, qc_block) = self
                    .cache
                    .pre_commits
                    .iter()
                    .next()
                    .expect("pre-commit should not be empty");

                assert_eq!(
                    qc_block.prev_block_id().unwrap(),
                    block.block_id().object_id(),
                    "the prev-block for all pre-commits should be the header"
                );

                (block, qc_block.qc().as_ref().unwrap())
            }
            None => {
                return Err(BuckyError::new(
                    BuckyErrorCode::NotFound,
                    "the header block is none",
                ));
            }
        };

        let mut parent_state_id = match header_block.result_state_id() {
            Some(state_id) => state_id.clone(),
            None => {
                return Ok(GroupRPathStatus {
                    block_desc: header_block.named_object().desc().clone(),
                    certificate: qc.clone(),
                    status_map: HashMap::new(),
                })
            }
        };

        let mut status_map = HashMap::new();

        let root_cache = self.storage_engine.root_cache();
        let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());

        for folder in sub_path.split(STATE_PATH_SEPARATOR) {
            let parent_state = self.non_driver.get_object(&parent_state_id, None).await?;

            if ObjectTypeCode::ObjectMap != parent_state.object().obj_type_code() {
                let msg = format!(
                    "unmatch object type at path {} in folder {}, expect: ObjectMap, got: {:?}",
                    sub_path,
                    folder,
                    parent_state.object().obj_type_code()
                );
                log::warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }

            let (parent, remain) = ObjectMap::raw_decode(parent_state.object_raw.as_slice())
                .map_err(|err| {
                    let msg = format!(
                        "decode failed at path {} in folder {}, {:?}",
                        sub_path, folder, err
                    );
                    log::warn!("{}", msg);
                    BuckyError::new(err.code(), msg)
                })?;

            assert_eq!(remain.len(), 0);

            status_map.insert(parent_state_id, parent_state);

            let sub_map_id = parent.get_by_key(&cache, folder).await?;
            match sub_map_id {
                Some(sub_map_id) => {
                    // for next folder
                    parent_state_id = sub_map_id;
                }
                None => {
                    return Ok(GroupRPathStatus {
                        block_desc: header_block.named_object().desc().clone(),
                        certificate: qc.clone(),
                        status_map,
                    });
                }
            }
        }

        let leaf_state = self.non_driver.get_object(&parent_state_id, None).await?;
        status_map.insert(parent_state_id, leaf_state);

        return Ok(GroupRPathStatus {
            block_desc: header_block.named_object().desc().clone(),
            certificate: qc.clone(),
            status_map,
        });
    }

    pub fn get_object_map_processor(&self) -> &dyn GroupObjectMapProcessor {
        &self.object_map_processor
    }
}
