use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoWData {
    pub object_id: ObjectId,
    pub difficulty: u8, // target difficulty
    pub nonce: Option<u128>,
}

impl PoWData {
    pub fn check_complete(&mut self, private_key: Vec<PrivateKey>) -> bool {
        if let Some(nonce) = &self.nonce {
            let builder = NonceBuilder::new(private_key);
            let diff = builder.calc_difficulty(&self.object_id, *nonce).unwrap();
            if diff < self.difficulty {
                error!("unmatched difficulty for current nonce! data={:?}, got difficulty={}", self, diff);
                self.nonce = None;
                false
            } else {
                true
            }
        } else {
            false
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoWThreadState {
    pub data: PoWData,
    pub id: u32,
    pub range: Range<u128>,
}

impl PoWThreadState {
    pub fn new(data: PoWData, id: u32) -> Self {
        let start: u128 = 0;
        let end: u128 = u128::MAX;

        let id_bytes = id.to_be_bytes();
        let mut bytes = start.to_be_bytes();
        bytes[..4].copy_from_slice(&id_bytes);
        let start = u128::from_be_bytes(bytes);

        let mut bytes = end.to_be_bytes();
        bytes[..4].copy_from_slice(&id_bytes);
        let end = u128::from_be_bytes(bytes);

        Self {
            data,
            id,
            range: start..end,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoWState {
    pub data: PoWData,
    pub id_range: Range<u32>,
    pub finished: HashSet<u32>,
    pub threads: Vec<PoWThreadState>,
}

impl PoWState {
    pub fn new(object_id: ObjectId, difficulty: u8, id_range: Range<u32>) -> Self {
        Self {
            data: PoWData {
                object_id,
                difficulty,
                nonce: None,
            },
            id_range,
            finished: HashSet::new(),
            threads: vec![],
        }
    }
}


#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PowThreadStatus {
    Sync,
    Finished,
}

pub trait PoWThreadStateSync: Send + Sync {
    fn private_key(&self) -> Vec<PrivateKey>;
    fn state(&self) -> PoWState;
    fn select(&self) -> Option<PoWThreadState>;
    fn sync(&self, state: &PoWThreadState, status: PowThreadStatus) -> bool;  // return true will continue; false will stop
}

pub type PoWThreadStateSyncRef = Arc<Box<dyn PoWThreadStateSync>>;

#[async_trait::async_trait]
pub trait PoWStateStorage: Send + Sync {
    async fn load(&self, data: &PoWData) -> BuckyResult<Option<PoWState>>;
    async fn save(&self, data: &PoWState) -> BuckyResult<()>;
}

pub type PoWStateStorageRef = Arc<Box<dyn PoWStateStorage>>;