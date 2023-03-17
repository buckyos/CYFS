use crate::object_pack::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::path::Path;

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
    pub object_files: Vec<ObjectPackFileInfo>,
    pub chunk_files: Vec<ObjectPackFileInfo>,
}

impl ObjectArchiveIndex {
    pub fn new(id: String, format: ObjectPackFormat, strategy: ObjectBackupStrategy) -> Self {
        let datetime = chrono::offset::Local::now();
        let time = format!("{:?}", datetime);

        Self {
            id,
            time,
            format,
            strategy,
            object_files: vec![],
            chunk_files: vec![],
        }
    }

    pub async fn load(dir: &Path) -> BuckyResult<Self> {
        let index_file = dir.join("index");
        let s = async_std::fs::read_to_string(&index_file)
            .await
            .map_err(|e| {
                let msg = format!(
                    "load index info from file failed! file={}, {}",
                    index_file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let ret: Self = serde_json::from_str(&s).map_err(|e| {
            let msg = format!(
                "invalid index info format! file={}, content={}, {}",
                index_file.display(),
                s,
                e,
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        debug!(
            "load backup archive index: file={}, {}",
            index_file.display(),
            s
        );
        Ok(ret)
    }

    pub async fn save(&self, dir: &Path) -> BuckyResult<()> {
        let index_file = dir.join("index");

        let data = serde_json::to_string_pretty(&self).unwrap();
        async_std::fs::write(&index_file, &data)
            .await
            .map_err(|e| {
                let msg = format!(
                    "write backup index info to file failed! file={}, {}",
                    index_file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        info!(
            "save backup index success! index={}, file={}",
            data,
            index_file.display()
        );
        Ok(())
    }
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