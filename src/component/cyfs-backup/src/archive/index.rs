use crate::crypto::*;
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

    pub device_id: DeviceId,
    pub owner: Option<ObjectId>,

    pub crypto: CryptoMode,
    pub en_device_id: Option<String>,

    pub object_files: Vec<ObjectPackFileInfo>,
    pub chunk_files: Vec<ObjectPackFileInfo>,

    pub meta: Option<serde_json::Value>,
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

            device_id: DeviceId::default(),
            owner: None,
            crypto: CryptoMode::None,
            en_device_id: None,

            object_files: vec![],
            chunk_files: vec![],
            meta: None,
        }
    }

    pub fn init_device_id(&mut self, device_id: DeviceId, owner: Option<ObjectId>, crypto: Option<&AesKey>) {
        let mode;
        let en_device_id;
        match crypto {
            Some(aes_key) => {
                mode = CryptoMode::AES;
                en_device_id = Some(AesKeyHelper::encrypt_device_id(&aes_key, &device_id));
            }
            None => {
                mode = CryptoMode::None;
                en_device_id = None;
            }
        }

        self.device_id = device_id;
        self.owner = owner;
        self.crypto = mode;
        self.en_device_id = en_device_id;
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
