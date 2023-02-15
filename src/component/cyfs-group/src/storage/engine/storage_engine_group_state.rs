use std::{collections::HashSet, sync::Arc};

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, ObjectId, ObjectMapPathOpEnvRef,
    ObjectMapRootManagerRef, ObjectMapSimpleContentType, ObjectMapSingleOpEnvRef, OpEnvPathAccess,
};

use crate::StatePath;

use super::{GroupStatePath, StorageEngine, StorageWriter, GROUP_STATE_PATH_BLOCK};

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
            .get_by_path(self.state_path.commit_block(height).as_str())
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
}

impl StorageEngineGroupStateWriter {
    async fn new(
        state_mgr: ObjectMapRootManagerRef,
        state_path: Arc<GroupStatePath>,
    ) -> BuckyResult<Self> {
        let op_env = state_mgr.create_op_env(ACCESS)?;
        let prepare_map_id = op_env.get_by_path(state_path.prepares()).await?;
        let prepare_op_env = state_mgr.create_single_op_env(ACCESS)?;
        match prepare_map_id.as_ref() {
            Some(prepare_map_id) => prepare_op_env.load(prepare_map_id).await?,
            None => {
                prepare_op_env
                    .create_new(ObjectMapSimpleContentType::Map)
                    .await?
            }
        };

        Ok(Self {
            op_env,
            prepare_op_env,
            state_path,
            state_mgr,
            prepare_map_id,
        })
    }
}

#[async_trait::async_trait]
impl StorageWriter for StorageEngineGroupStateWriter {
    async fn insert_prepares(&mut self, block_id: &ObjectId) -> BuckyResult<()> {
        let new_prepare = self.state_mgr.create_single_op_env(ACCESS)?;
        new_prepare
            .create_new(ObjectMapSimpleContentType::Map)
            .await?;
        new_prepare
            .insert_with_key(GROUP_STATE_PATH_BLOCK, block_id)
            .await?;
        let new_prepare_block = new_prepare.commit().await?;

        self.prepare_op_env
            .insert_with_key(block_id.to_string().as_str(), &new_prepare_block)
            .await
    }

    async fn insert_pre_commit(
        &mut self,
        block_id: &ObjectId,
        is_instead: bool,
    ) -> BuckyResult<()> {
        if !self
            .prepare_op_env
            .remove_with_key(block_id.to_string().as_str())
            .await?
            .is_some()
        {
            assert!(false);
        }

        // TODO
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

    async fn push_commit(&mut self, height: u64, block_id: &ObjectId) -> BuckyResult<()> {
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
        timestamp: Option<u64>,
    ) -> BuckyResult<()> {
        if timestamp - self.engine.finish_proposals.flip_timestamp > 60000 {
            let mut new_over = HashSet::new();
            std::mem::swap(&mut new_over, &mut self.engine.finish_proposals.adding);
            std::mem::swap(&mut new_over, &mut self.engine.finish_proposals.over);
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

    async fn set_last_vote_round(&mut self, round: u64) -> BuckyResult<()> {
        self.engine.last_vote_round = round;

        Ok(())
    }
}

impl<'a> Drop for StorageEngineGroupStateWriter<'a> {
    fn drop(&mut self) {}
}
