use crate::object_pack::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::path::Path;


#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveIndex {
    pub id: u64,
    pub time: String,
    pub format: ObjectPackFormat,
    pub object_files: Vec<ObjectPackFileInfo>,
    pub chunk_files: Vec<ObjectPackFileInfo>,
}

impl ObjectArchiveIndex {
    pub fn new(id: u64, format: ObjectPackFormat) -> Self {
        let datetime = chrono::offset::Local::now();
        // let time = datetime.format("%Y-%m-%d %H:%M:%S%.3f %:z");
        let time = format!("{:?}", datetime);

        Self {
            id,
            time,
            format,
            object_files: vec![],
            chunk_files: vec![],
        }
    }

    pub async fn load(meta_file: &Path) -> BuckyResult<Self> {
        let s = async_std::fs::read_to_string(meta_file)
            .await
            .map_err(|e| {
                let msg = format!(
                    "load meta info from file failed! file={}, {}",
                    meta_file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let ret: Self = serde_json::from_str(&s).map_err(|e| {
            let msg = format!(
                "invalid meta info format! file={}, meta={}, {}",
                meta_file.display(),
                s,
                e,
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        Ok(ret)
    }

    pub async fn save(&self, meta_file: &Path) -> BuckyResult<()> {
        let meta = serde_json::to_string_pretty(&self).unwrap();
        async_std::fs::write(&meta_file, meta).await.map_err(|e| {
            let msg = format!(
                "write meta info to file failed! file={}, {}",
                meta_file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

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
