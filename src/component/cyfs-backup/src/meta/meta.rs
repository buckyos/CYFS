use cyfs_base::*;
use super::data::KeyDataMeta;

use serde::{Deserialize, Serialize};
use std::path::Path;


#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveMeta<T> {
    pub id: u64,
    pub time: String,

    pub object: T,
    pub key_data: Vec<KeyDataMeta>
}

impl<T> ObjectArchiveMeta<T>
where
    T: Default + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(id: u64) -> Self {
        let datetime = chrono::offset::Local::now();
        let time = format!("{:?}", datetime);

        Self {
            id,
            time,
            object: T::default(),
            key_data: vec![],
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
