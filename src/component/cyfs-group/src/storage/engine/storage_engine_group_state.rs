use std::{collections::HashSet, sync::Arc};

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, ObjectId, ObjectIdDataBuilder, ObjectMapPathOpEnvRef,
    ObjectMapRootManagerRef, ObjectMapSimpleContentType, ObjectMapSingleOpEnvRef, OpEnvPathAccess,
};

use crate::{
    GroupStatePath, GROUP_STATE_PATH_DEC_STATE, GROUP_STATE_PATH_FLIP_TIME,
    GROUP_STATE_PATH_LAST_VOTE_ROUNDS, GROUP_STATE_PATH_RANGE,
};

use super::{StorageEngine, StorageWriter};

const ACCESS: Option<OpEnvPathAccess> = None;

#[derive(Clone)]
pub struct StorageEngineGroupState {
    state_mgr: ObjectMapRootManagerRef,
    state_path: Arc<GroupStatePath>,
}

impl StorageEngineGroupState {
    pub async fn load(
        dec_group_state: ObjectMapRootManagerRef,
        state_path: GroupStatePath,
    ) -> BuckyResult<StorageEngineGroupState> {
        Ok(Self {
            state_mgr: todo!(),
            state_path: Arc::new(state_path),
        })
    }

    pub async fn create_writer(&self) -> BuckyResult<StorageEngineGroupStateWriter> {
        Ok(
            StorageEngineGroupStateWriter::new(self.state_mgr.clone(), self.state_path.clone())
                .await?,
        )
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
                        .create_new(ObjectMapSimpleContentType::Set)
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

    async fn insert_prepares_inner(&mut self, block_id: &ObjectId) -> BuckyResult<()> {
        self.prepare_op_env
            .insert(block_id)
            .await
            .map(|is_changed| assert!(is_changed))
    }

    async fn insert_pre_commit_inner(
        &mut self,
        block_id: &ObjectId,
        is_instead: bool,
    ) -> BuckyResult<()> {
        let is_changed = self.prepare_op_env.remove(block_id).await?;
        assert!(is_changed);

        if is_instead {
            self.op_env
                .remove_with_path(self.state_path.pre_commits(), &None)
                .await?;
        }

        self.op_env
            .insert_with_path(self.state_path.pre_commits(), block_id)
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

        let range_obj = make_range_obj(min_height, height);
        if height == 1 {
            self.op_env
                .insert_with_key(self.state_path.link(), GROUP_STATE_PATH_RANGE, &range_obj)
                .await?;
        } else {
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
            let is_changed = self.prepare_op_env.remove(block_id).await?;
            assert!(is_changed);
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
}

#[async_trait::async_trait]
impl StorageWriter for StorageEngineGroupStateWriter {
    async fn insert_prepares(&mut self, block_id: &ObjectId) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self.insert_prepares_inner(block_id).await;
        self.write_result.clone()
    }

    async fn insert_pre_commit(
        &mut self,
        block_id: &ObjectId,
        is_instead: bool,
    ) -> BuckyResult<()> {
        self.write_result.as_ref().map_err(|e| e.clone())?;
        self.write_result = self.insert_pre_commit_inner(block_id, is_instead).await;
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
        self.op_env.commit().await.map(|_| ())
    }
}

fn make_range_obj(min: u64, max: u64) -> ObjectId {
    let mut range_buf = [0u8; 24];
    let (low, high) = range_buf.split_at_mut(12);
    low.copy_from_slice(&min.to_le_bytes());
    high.copy_from_slice(&max.to_le_bytes());
    ObjectIdDataBuilder::new().data(&range_buf).build().unwrap()
}

fn make_u64_obj(value: u64) -> ObjectId {
    let mut range_buf = [0u8; 8];
    range_buf.copy_from_slice(&value.to_le_bytes());
    ObjectIdDataBuilder::new().data(&range_buf).build().unwrap()
}
