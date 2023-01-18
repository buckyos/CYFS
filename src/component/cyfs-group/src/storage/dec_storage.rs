use std::sync::Arc;

use async_std::sync::RwLock;
use cyfs_base::{BuckyResult, ObjectId};
use cyfs_core::{GroupConsensusBlock, GroupRPathStatus};

#[derive(Clone)]
pub struct DecStorageCache {
    pub state: Option<ObjectId>,
    pub header_block: GroupConsensusBlock,
    pub qc_block: GroupConsensusBlock,
}

// TODO: storage

#[derive(Clone)]
pub struct DecStorage {
    cache: Arc<RwLock<Option<DecStorageCache>>>,
}

impl DecStorage {
    pub async fn load() -> BuckyResult<Self> {
        unimplemented!();
        let obj = Self {
            cache: Arc::new(RwLock::new(None)),
        };

        Ok(obj)
    }

    pub async fn cur_state(&self) -> Option<DecStorageCache> {
        let cur = self.cache.read().await;
        (*cur).clone()
    }

    pub async fn sync(
        &self,
        header_block: &GroupConsensusBlock,
        qc_block: &GroupConsensusBlock,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn get_by_path(&self, path: &str) -> BuckyResult<GroupRPathStatus> {
        unimplemented!()
    }
}
