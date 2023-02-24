use std::{collections::HashSet, sync::Arc};

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, ObjectId, ObjectIdDataBuilder, ObjectMapContentItem,
    ObjectMapIsolatePathOpEnvRef, ObjectMapPathOpEnvRef, ObjectMapRootCacheRef,
    ObjectMapRootManagerRef, ObjectMapSimpleContentType, ObjectMapSingleOpEnvRef, OpEnvPathAccess,
};
use cyfs_core::{GroupConsensusBlockObject, HotstuffBlockQC, HotstuffTimeout};

use crate::{
    GroupObjectMapProcessor, GroupStatePath, NONDriverHelper, GROUP_STATE_PATH_BLOCK,
    GROUP_STATE_PATH_DEC_STATE, GROUP_STATE_PATH_FLIP_TIME, GROUP_STATE_PATH_RANGE,
    GROUP_STATE_PATH_RESULT_STATE,
};

use super::{StorageCacheInfo, StorageEngine, StorageWriter};

const ACCESS: Option<OpEnvPathAccess> = None;

#[derive(Clone)]
pub struct StorageEngineGroupState {
    state_mgr: ObjectMapRootManagerRef,
    state_path: Arc<GroupStatePath>,
}

impl StorageEngineGroupState {
    pub(crate) async fn load_cache(
        state_mgr: &ObjectMapRootManagerRef,
        non_driver: &NONDriverHelper,
        state_path: &GroupStatePath,
    ) -> BuckyResult<StorageCacheInfo> {
        let op_env = state_mgr.create_op_env(ACCESS).map_err(|err| {
            log::warn!("create_op_env failed {:?}", err);
            err
        })?;

        let dec_state_id = op_env.get_by_path(state_path.dec_state()).await;
        let dec_state_id = map_not_found_option_to_option(dec_state_id)?;

        let last_vote_round = op_env.get_by_path(state_path.last_vote_round()).await;
        let last_vote_round =
            map_not_found_option_to_option(last_vote_round)?.map(|id| parse_u64_obj(&id));

        let last_qc = op_env.get_by_path(state_path.last_qc()).await;
        let last_qc = map_not_found_option_to_option(last_qc)?;
        let last_qc = match last_qc.as_ref() {
            Some(qc_id) => non_driver
                .get_qc(qc_id, None)
                .await?
                .try_into()
                .map_or(None, |qc| Some(qc)),
            None => None,
        };

        let last_tc = op_env.get_by_path(state_path.last_tc()).await;
        let last_tc = map_not_found_option_to_option(last_tc)?;
        let last_tc = match last_tc.as_ref() {
            Some(tc_id) => non_driver
                .get_qc(tc_id, None)
                .await?
                .try_into()
                .map_or(None, |tc| Some(tc)),
            None => None,
        };

        let mut first_header_block_ids: Vec<ObjectId> = vec![];
        let commit_range = op_env.get_by_path(state_path.range()).await;
        let commit_range =
            map_not_found_option_to_option(commit_range)?.map(|id| parse_range_obj(&id));
        let commit_block = match commit_range {
            Some((first_height, header_height)) => {
                let first_block_id = op_env
                    .get_by_path(state_path.commit_height(first_height).as_str())
                    .await;
                let first_block_id =
                    map_not_found_option_to_option(first_block_id)?.expect("first block is lost");
                first_header_block_ids.push(first_block_id);

                if header_height == first_height {
                    Some((first_block_id, first_block_id))
                } else {
                    let header_block_id = op_env
                        .get_by_path(state_path.commit_height(header_height).as_str())
                        .await;
                    let header_block_id = map_not_found_option_to_option(header_block_id)?
                        .expect("first block is lost");
                    first_header_block_ids.push(header_block_id);
                    Some((first_block_id, header_block_id))
                }
            }
            None => None,
        };

        let prepare_block_ids =
            load_object_ids_with_path_map_key(&op_env, state_path.prepares()).await?;
        if prepare_block_ids.len() == 0 && commit_range.is_none() {
            return Err(BuckyError::new(
                BuckyErrorCode::NotFound,
                "not found in storage",
            ));
        }

        let pre_commit_block_ids =
            load_object_ids_with_path_map_key(&op_env, state_path.pre_commits()).await?;

        let flip_timestamp = op_env.get_by_path(state_path.flip_time()).await;
        let flip_timestamp = map_not_found_option_to_option(flip_timestamp)?.map_or(0, |id| {
            let n = parse_u64_obj(&id);
            // log::debug!(
            //     "load flip timestamp {}/{} -> {}",
            //     id,
            //     id.to_hex().unwrap(),
            //     n
            // );
            n
        });

        let adding_proposal_ids =
            load_object_ids_with_path_set(&op_env, state_path.adding()).await?;
        let over_proposal_ids =
            load_object_ids_with_path_set(&op_env, state_path.recycle()).await?;

        let load_block_ids = [
            first_header_block_ids.as_slice(),
            prepare_block_ids.as_slice(),
            pre_commit_block_ids.as_slice(),
        ]
        .concat();

        let load_blocks = futures::future::join_all(load_block_ids.iter().map(|id| async {
            let id = id.clone();
            non_driver.get_block(&id, None).await.map_err(|err| {
                log::warn!("get block {} failed {:?}", id, err);
                err
            })
        }))
        .await;

        let mut cache = StorageCacheInfo::new(dec_state_id);
        cache.last_vote_round = last_vote_round.map_or(0, |round| round);
        cache.last_qc = last_qc;
        cache.last_tc = last_tc;
        cache.finish_proposals.adding = HashSet::from_iter(adding_proposal_ids.into_iter());
        cache.finish_proposals.over = HashSet::from_iter(over_proposal_ids.into_iter());
        cache.finish_proposals.flip_timestamp = flip_timestamp;

        let prepare_block_pos = match commit_block {
            Some((first_block_id, header_block_id)) => {
                cache.first_block = Some(load_blocks.get(0).unwrap().clone()?);
                if header_block_id == first_block_id {
                    cache.header_block = cache.first_block.clone();
                    1
                } else {
                    cache.header_block = Some(load_blocks.get(1).unwrap().clone()?);
                    2
                }
            }
            None => 0,
        };

        let dec_state_id_in_header = cache
            .header_block
            .as_ref()
            .map_or(None, |b| b.result_state_id().clone());

        assert_eq!(dec_state_id, dec_state_id_in_header);
        if dec_state_id != dec_state_id_in_header {
            return Err(BuckyError::new(
                BuckyErrorCode::Unmatch,
                "the state should same as it in header-block",
            ));
        }

        let (prepare_blocks, pre_commit_blocks) =
            load_blocks.as_slice()[prepare_block_pos..].split_at(prepare_block_ids.len());
        for (block, block_id) in prepare_blocks.iter().zip(prepare_block_ids) {
            cache.prepares.insert(block_id, block.clone()?);
        }
        for (block, block_id) in pre_commit_blocks.iter().zip(pre_commit_block_ids) {
            cache.pre_commits.insert(block_id, block.clone()?);
        }

        Ok(cache)
    }

    pub fn new(state_mgr: ObjectMapRootManagerRef, state_path: GroupStatePath) -> Self {
        Self {
            state_mgr,
            state_path: Arc::new(state_path),
        }
    }

    pub async fn create_writer(&self) -> BuckyResult<StorageEngineGroupStateWriter> {
        Ok(
            StorageEngineGroupStateWriter::new(self.state_mgr.clone(), self.state_path.clone())
                .await?,
        )
    }

    pub fn root_cache(&self) -> &ObjectMapRootCacheRef {
        self.state_mgr.root_cache()
    }
}

#[async_trait::async_trait]
impl StorageEngine for StorageEngineGroupState {
    async fn find_block_by_height(&self, height: u64) -> BuckyResult<ObjectId> {
        let op_env = self.state_mgr.create_op_env(ACCESS)?;
        let block_id = op_env
            .get_by_path(self.state_path.commit_height(height).as_str())
            .await?;
        block_id.map_or(
            Err(BuckyError::new(BuckyErrorCode::NotFound, "not found")),
            |block_id| Ok(block_id),
        )
    }
}

#[derive(Clone)]
pub struct StorageEngineGroupStateWriter {
    state_mgr: ObjectMapRootManagerRef,
    op_env: ObjectMapPathOpEnvRef,
    prepare_op_env: ObjectMapSingleOpEnvRef,
    prepare_map_id: Option<ObjectId>,
    state_path: Arc<GroupStatePath>,
    write_result: BuckyResult<()>,
}

impl StorageEngineGroupStateWriter {
    async fn new(
        state_mgr: ObjectMapRootManagerRef,
        state_path: Arc<GroupStatePath>,
    ) -> BuckyResult<Self> {
        let op_env = state_mgr.create_op_env(ACCESS)?;
        let prepare_op_env = state_mgr.create_single_op_env(ACCESS)?;
        let prepare_map_id =
            if let Err(err) = prepare_op_env.load_by_path(state_path.prepares()).await {
                if err.code() == BuckyErrorCode::NotFound {
                    prepare_op_env
                        .create_new(ObjectMapSimpleContentType::Map)
                        .await?;
                    None
                } else {
                    return Err(err);
                }
            } else {
                prepare_op_env.get_current_root().await
            };

        Ok(Self {
            op_env,
            prepare_op_env,
            state_path,
            state_mgr,
            prepare_map_id,
            write_result: Ok(()),
        })
    }

    async fn create_block_result_object_map(
        &self,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
    ) -> BuckyResult<ObjectId> {
        let single_op_env = self.state_mgr.create_single_op_env(ACCESS)?;
        single_op_env
            .create_new(ObjectMapSimpleContentType::Map)
            .await?;
        single_op_env
            .insert_with_key(GROUP_STATE_PATH_BLOCK, block_id)
            .await?;
        if let Some(state_id) = result_state_id.as_ref() {
            single_op_env
                .insert_with_key(GROUP_STATE_PATH_RESULT_STATE, state_id)
                .await?;
        }

        single_op_env.commit().await
    }

    async fn insert_prepares_inner(
        &mut self,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
    ) -> BuckyResult<()> {
        let block_result_pair = self
            .create_block_result_object_map(block_id, result_state_id)
            .await?;
        self.prepare_op_env
            .insert_with_key(block_id.to_string().as_str(), &block_result_pair)
            .await
    }

    async fn insert_pre_commit_inner(
        &mut self,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
        is_instead: bool,
    ) -> BuckyResult<()> {
        let block_result_pair = self
            .prepare_op_env
            .remove_with_key(block_id.to_string().as_str(), &None)
            .await?;
        assert!(block_result_pair.is_some());

        if is_instead {
            self.op_env
                .remove_with_path(self.state_path.pre_commits(), &None)
                .await?;
        }

        let block_result_pair = self
            .create_block_result_object_map(block_id, result_state_id)
            .await?;

        self.op_env
            .insert_with_key(
                self.state_path.pre_commits(),
                block_id.to_string().as_str(),
                &block_result_pair,
            )
            .await
    }

    async fn push_commit_inner(
        &mut self,
        height: u64,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
        prev_result_state_id: &Option<ObjectId>,
        min_height: u64,
    ) -> BuckyResult<()> {
        self.op_env
            .insert_with_path(self.state_path.commit_height(height).as_str(), block_id)
            .await?;

        if height == 1 {
            let range_obj = make_range_obj(1, height);
            self.op_env
                .insert_with_key(self.state_path.link(), GROUP_STATE_PATH_RANGE, &range_obj)
                .await?;
        } else {
            assert!(min_height < height);
            let range_obj = make_range_obj(min_height, height);
            let prev_range = make_range_obj(min_height, height - 1);
            let prev_value = self
                .op_env
                .set_with_key(
                    self.state_path.link(),
                    GROUP_STATE_PATH_RANGE,
                    &range_obj,
                    &Some(prev_range),
                    false,
                )
                .await?;
            assert_eq!(prev_value.unwrap(), prev_range);
        };

        // update state from dec-app
        if result_state_id == prev_result_state_id {
            return Ok(());
        } else {
            match result_state_id {
                Some(result_state_id) => {
                    if prev_result_state_id.is_none() {
                        self.op_env
                            .insert_with_key(
                                self.state_path.root(),
                                GROUP_STATE_PATH_DEC_STATE,
                                result_state_id,
                            )
                            .await?;
                    } else {
                        let prev_value = self
                            .op_env
                            .set_with_key(
                                self.state_path.root(),
                                GROUP_STATE_PATH_DEC_STATE,
                                result_state_id,
                                prev_result_state_id,
                                false,
                            )
                            .await?;
                        assert_eq!(&prev_value, prev_result_state_id);
                    }
                }
                None => {
                    self.op_env
                        .remove_with_path(self.state_path.dec_state(), prev_result_state_id)
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn remove_prepares_inner(&mut self, block_ids: &[ObjectId]) -> BuckyResult<()> {
        for block_id in block_ids {
            let block_result_pair = self
                .prepare_op_env
                .remove_with_key(block_id.to_string().as_str(), &None)
                .await?;
            assert!(block_result_pair.is_some());
        }
        Ok(())
    }

    async fn push_proposals_inner(
        &mut self,
        proposal_ids: &[ObjectId],
        timestamp: Option<(u64, u64)>, // (timestamp, prev_timestamp), 0 if the first
    ) -> BuckyResult<()> {
        if proposal_ids.is_empty() {
            return Ok(());
        }

        let add_single_op_env = self.state_mgr.create_single_op_env(ACCESS)?;

        if let Some((timestamp, prev_timestamp)) = timestamp {
            let new_over = self
                .op_env
                .remove_with_path(self.state_path.adding(), &None)
                .await?;

            if let Some(new_over) = new_over.as_ref() {
                self.op_env
                    .set_with_path(self.state_path.recycle(), new_over, &None, true)
                    .await?;
            }

            let timestamp_obj = make_u64_obj(timestamp);
            if prev_timestamp != 0 {
                let prev_timestamp_obj = make_u64_obj(prev_timestamp);
                // log::debug!(
                //     "will update flip-time from {} -> {}/{} to {} -> {}/{}",
                //     prev_timestamp,
                //     prev_timestamp_obj,
                //     prev_timestamp_obj.to_hex().unwrap(),
                //     timestamp,
                //     timestamp_obj,
                //     timestamp_obj.to_hex().unwrap(),
                // );
                let prev_value = self
                    .op_env
                    .set_with_path(
                        self.state_path.flip_time(),
                        &timestamp_obj,
                        &Some(prev_timestamp_obj),
                        false,
                    )
                    .await?;
                assert_eq!(prev_value.unwrap(), prev_timestamp_obj);
            } else {
                // log::debug!("will update flip-time from None to {}", timestamp);
                self.op_env
                    .insert_with_key(
                        self.state_path.finish_proposals(),
                        GROUP_STATE_PATH_FLIP_TIME,
                        &timestamp_obj,
                    )
                    .await?;
            }

            add_single_op_env
                .create_new(ObjectMapSimpleContentType::Set)
                .await?;
        } else {
            add_single_op_env
                .load_by_path(self.state_path.adding())
                .await?;
        }

        for proposal_id in proposal_ids {
            let is_new = add_single_op_env.insert(proposal_id).await?;
            assert!(is_new);
        }
        let adding_set_id = add_single_op_env.commit().await?;
        let prev_value = self
            .op_env
            .set_with_path(self.state_path.adding(), &adding_set_id, &None, true)
            .await?;

        Ok(())
    }

    async fn set_last_vote_round_inner(&mut self, round: u64, prev_value: u64) -> BuckyResult<()> {
        assert!(round > prev_value);
        if round == prev_value {
            return Ok(());
        }

        let round_obj = make_u64_obj(round);

        if prev_value == 0 {
            self.op_env
                .insert_with_path(self.state_path.last_vote_round(), &round_obj)
                .await
        } else {
            let prev_obj = make_u64_obj(prev_value);
            let prev_value = self
                .op_env
                .set_with_path(
                    self.state_path.last_vote_round(),
                    &round_obj,
                    &Some(prev_obj),
                    false,
                )
                .await?;
            assert_eq!(prev_value.unwrap(), prev_obj);
            Ok(())
        }
    }

    async fn save_last_qc_inner(&mut self, qc_id: &ObjectId) -> BuckyResult<()> {
        self.op_env
            .set_with_path(self.state_path.last_qc(), qc_id, &None, true)
            .await
            .map(|_| ())
    }

    async fn save_last_tc_inner(&mut self, tc_id: &ObjectId) -> BuckyResult<()> {
        self.op_env
            .set_with_path(self.state_path.last_tc(), tc_id, &None, true)
            .await
            .map(|_| ())
    }
}

#[async_trait::async_trait]
impl StorageWriter for StorageEngineGroupStateWriter {
    async fn insert_prepares(
        &mut self,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
    ) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self.insert_prepares_inner(block_id, result_state_id).await;
        self.write_result.clone()
    }

    async fn insert_pre_commit(
        &mut self,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
        is_instead: bool,
    ) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self
            .insert_pre_commit_inner(block_id, result_state_id, is_instead)
            .await;
        self.write_result.clone()
    }

    async fn push_commit(
        &mut self,
        height: u64,
        block_id: &ObjectId,
        result_state_id: &Option<ObjectId>,
        prev_result_state_id: &Option<ObjectId>,
        min_height: u64,
    ) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self
            .push_commit_inner(
                height,
                block_id,
                result_state_id,
                prev_result_state_id,
                min_height,
            )
            .await;
        self.write_result.clone()
    }

    async fn remove_prepares(&mut self, block_ids: &[ObjectId]) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self.remove_prepares_inner(block_ids).await;
        self.write_result.clone()
    }

    async fn push_proposals(
        &mut self,
        proposal_ids: &[ObjectId],
        timestamp: Option<(u64, u64)>, // (timestamp, prev_timestamp), 0 if the first
    ) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self.push_proposals_inner(proposal_ids, timestamp).await;
        self.write_result.clone()
    }

    async fn set_last_vote_round(&mut self, round: u64, prev_value: u64) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self.set_last_vote_round_inner(round, prev_value).await;
        self.write_result.clone()
    }

    async fn save_last_qc(&mut self, qc_id: &ObjectId) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self.save_last_qc_inner(qc_id).await;
        self.write_result.clone()
    }

    async fn save_last_tc(&mut self, tc_id: &ObjectId) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self.save_last_tc_inner(tc_id).await;
        self.write_result.clone()
    }

    async fn commit(mut self) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;

        let prepare_map_id = self.prepare_op_env.commit().await?;
        self.op_env
            .set_with_path(
                self.state_path.prepares(),
                &prepare_map_id,
                &self.prepare_map_id,
                self.prepare_map_id.is_none(),
            )
            .await?;
        self.op_env.commit().await.map_or_else(
            |err| {
                if err.code() == BuckyErrorCode::AlreadyExists {
                    Ok(())
                } else {
                    Err(err)
                }
            },
            |_| Ok(()),
        )
    }
}

fn make_range_obj(min: u64, max: u64) -> ObjectId {
    let mut range_buf = [0u8; 24];
    let (low, high) = range_buf.split_at_mut(12);
    low[..8].copy_from_slice(&min.to_le_bytes());
    high[..8].copy_from_slice(&max.to_le_bytes());
    ObjectIdDataBuilder::new().data(&range_buf).build().unwrap()
}

fn parse_range_obj(obj: &ObjectId) -> (u64, u64) {
    let range_buf = obj.data();
    assert_eq!(range_buf.len(), 24);
    let (low_buf, high_buf) = range_buf.split_at(12);
    let mut low = [0u8; 8];
    low.copy_from_slice(&low_buf[..8]);
    let mut high = [0u8; 8];
    high.copy_from_slice(&high_buf[..8]);

    (u64::from_le_bytes(low), u64::from_le_bytes(high))
}

fn make_u64_obj(value: u64) -> ObjectId {
    let mut range_buf = [0u8; 8];
    range_buf.copy_from_slice(&value.to_le_bytes());
    ObjectIdDataBuilder::new().data(&range_buf).build().unwrap()
}

fn parse_u64_obj(obj: &ObjectId) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(obj.data());
    u64::from_le_bytes(buf)
}

async fn load_object_ids_with_path_set(
    op_env: &ObjectMapPathOpEnvRef,
    full_path: &str,
) -> BuckyResult<Vec<ObjectId>> {
    let content = match op_env.list(full_path).await {
        Ok(content) => content,
        Err(err) => {
            log::warn!("list by path {} failed {:?}", full_path, err);
            if err.code() == BuckyErrorCode::NotFound {
                return Ok(vec![]);
            } else {
                return Err(err);
            }
        }
    };

    let mut object_ids: Vec<ObjectId> = vec![];
    for item in content.list.iter() {
        match item {
            ObjectMapContentItem::Set(id) => object_ids.push(id.clone()),
            _ => {
                log::error!("should be a set in path {}", full_path);
                return Err(BuckyError::new(
                    BuckyErrorCode::InvalidFormat,
                    format!("should be a set in path {}", full_path),
                ));
            }
        }
    }

    Ok(object_ids)
}

async fn load_object_ids_with_path_map_key(
    op_env: &ObjectMapPathOpEnvRef,
    full_path: &str,
) -> BuckyResult<Vec<ObjectId>> {
    let content = match op_env.list(full_path).await {
        Ok(content) => content,
        Err(err) => {
            log::warn!("list by path {} failed {:?}", full_path, err);
            if err.code() == BuckyErrorCode::NotFound {
                return Ok(vec![]);
            } else {
                return Err(err);
            }
        }
    };

    let mut object_ids: Vec<ObjectId> = vec![];
    for item in content.list.iter() {
        match item {
            ObjectMapContentItem::Map((key_id_base58, _)) => {
                object_ids.push(ObjectId::from_base58(key_id_base58)?)
            }
            _ => {
                log::error!("should be a set in path {}", full_path);
                return Err(BuckyError::new(
                    BuckyErrorCode::InvalidFormat,
                    format!("should be a set in path {}", full_path),
                ));
            }
        }
    }

    Ok(object_ids)
}

fn map_not_found_to_option<T>(r: BuckyResult<T>) -> BuckyResult<Option<T>> {
    match r {
        Ok(t) => Ok(Some(t)),
        Err(err) => {
            if err.code() == BuckyErrorCode::NotFound {
                Ok(None)
            } else {
                Err(err)
            }
        }
    }
}

fn map_not_found_option_to_option<T>(r: BuckyResult<Option<T>>) -> BuckyResult<Option<T>> {
    match r {
        Ok(t) => Ok(t),
        Err(err) => {
            if err.code() == BuckyErrorCode::NotFound {
                Ok(None)
            } else {
                Err(err)
            }
        }
    }
}

pub struct GroupObjectMapProcessorGroupState {
    state_mgr: ObjectMapRootManagerRef,
}

impl GroupObjectMapProcessorGroupState {
    pub fn new(state_mgr: &ObjectMapRootManagerRef) -> Self {
        Self {
            state_mgr: state_mgr.clone(),
        }
    }
}

#[async_trait::async_trait]
impl GroupObjectMapProcessor for GroupObjectMapProcessorGroupState {
    async fn create_single_op_env(&self) -> BuckyResult<ObjectMapSingleOpEnvRef> {
        self.state_mgr.create_single_op_env(ACCESS)
    }

    async fn create_sub_tree_op_env(&self) -> BuckyResult<ObjectMapIsolatePathOpEnvRef> {
        self.state_mgr.create_isolate_path_op_env(ACCESS)
    }
}
