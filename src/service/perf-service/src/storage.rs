use async_trait::async_trait;

use cyfs_base::{BuckyResult};

use cyfs_perf_base::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum  StorageType {
    MangoDB = 1,
}

impl Default for StorageType {
    fn default() -> Self {
        let store_type = StorageType::MangoDB;
        store_type
    }
}

impl std::fmt::Display for StorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match *self {

            #[cfg(feature = "mongo")]
            Self::MangoDB => "mongodb",
        };

        write!(f, "{}", msg)
    }
}

#[async_trait]
pub trait Storage: Sync + Send {
    async fn insert_entity_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()>;

    fn clone(&self) -> Box<dyn Storage>;

}










