use super::{data::KeyDataMeta, state::ObjectArchiveStateMeta, ObjectArchiveUniMeta};
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::path::Path;


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectArchiveMeta<T> {
    pub id: String,
    pub time: String,
    pub object: T,
    pub key_data: Vec<KeyDataMeta>,
}

impl<T> ObjectArchiveMeta<T>
where
    T: std::fmt::Debug + Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(id: String, object: T, key_data: Vec<KeyDataMeta>) -> Self {
        let datetime = chrono::offset::Local::now();
        let time = format!("{:?}", datetime);

        Self {
            id,
            time,
            object,
            key_data,
        }
    }

    pub async fn load(dir: &Path) -> BuckyResult<Self> {
        let meta_file = dir.join("meta");
        let s = async_std::fs::read_to_string(&meta_file)
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

    pub async fn save(&self, dir: &Path) -> BuckyResult<()> {
        let meta_file = dir.join("meta");
        let meta = serde_json::to_string_pretty(&self).unwrap();
        async_std::fs::write(&meta_file, &meta).await.map_err(|e| {
            let msg = format!(
                "write backup meta info to file failed! file={}, {}",
                meta_file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!("save backup meta success! meta={}, file={}", meta, meta_file.display());

        Ok(())
    }
}

pub type ObjectArchiveMetaForUniBackup = ObjectArchiveMeta<ObjectArchiveUniMeta>;
pub type ObjectArchiveMetaForStateBackup = ObjectArchiveMeta<ObjectArchiveStateMeta>;
