pub mod mongo;

use async_trait::async_trait;

use cyfs_base::{BuckyResult};

use cyfs_perf_base::*;
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize, Serializer};
use serde::ser::SerializeStruct;
use crate::storage::mongo::{MangodbStorage, MongoConfig};

#[derive(Serialize, Deserialize)]
pub(crate) struct StorageConfig {
    pub(crate) isolate: Option<String>,
    pub(crate) database: DatabaseConfig
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            isolate: Some("perf-service".to_owned()),
            database: DatabaseConfig::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all="lowercase")]
pub(crate) enum DatabaseConfig {
    MongoDB(MongoConfig)
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        DatabaseConfig::MongoDB(MongoConfig::default())
    }
}

impl Serialize for DatabaseConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut state = serializer.serialize_struct("database", 1)?;
        match self {
            DatabaseConfig::MongoDB(config) => {
                state.serialize_field("mongodb", config)?;
            }
        }
        state.end()
    }
}

#[async_trait]
pub(crate) trait Storage: Sync + Send {
    async fn insert_entity_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()>;
}

pub(crate) type StorageRef = Arc<dyn Storage>;

pub(crate) async fn create_storage(config: &StorageConfig) -> BuckyResult<StorageRef> {
    match &config.database {
        DatabaseConfig::MongoDB(mongo_config) => {
            let storage = MangodbStorage::new(config.isolate.as_deref(), mongo_config).await?;
            Ok(Arc::new(storage))
        }
    }
}










