use crate::crypto::*;
use crate::object_pack::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum ObjectBackupStrategy {
    State,
    Uni,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectArchiveIndex {
    pub id: String,
    pub time: String,

    pub format: ObjectPackFormat,
    pub strategy: ObjectBackupStrategy,

    pub device_id: DeviceId,
    pub owner: Option<ObjectId>,

    pub crypto: CryptoMode,
    pub en_device_id: Option<String>,

    pub object_files: Vec<ObjectPackFileInfo>,
    pub chunk_files: Vec<ObjectPackFileInfo>,

    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ObjectArchiveDataType {
    Object,
    Chunk,
}

impl ObjectArchiveDataType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Object => "object",
            Self::Chunk => "chunk",
        }
    }
}
