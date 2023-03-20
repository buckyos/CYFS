use super::{data::KeyDataMeta, state::ObjectArchiveStateMeta, ObjectArchiveUniMeta};
use cyfs_base::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectArchiveMeta<T> {
    pub object: T,
    pub key_data: Vec<KeyDataMeta>,
}

impl<T> ObjectArchiveMeta<T>
where
    T: std::fmt::Debug + Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(object: T, key_data: Vec<KeyDataMeta>) -> Self {
        Self { object, key_data }
    }

    pub fn load(value: serde_json::Value) -> BuckyResult<Self> {
        let s = serde_json::to_string(&value).unwrap();
        let ret: Self = serde_json::from_value(value).map_err(|e| {
            let msg = format!("invalid meta info format! meta={}, {}", s, e,);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        Ok(ret)
    }

    pub fn save(&self) -> BuckyResult<serde_json::Value> {
        serde_json::to_value(&self).map_err(|e| {
            let msg = format!("save meta to serde value faild! value={:?}, {}", self, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })
    }

    /*
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

        info!(
            "save backup meta success! meta={}, file={}",
            meta,
            meta_file.display()
        );

        Ok(())
    }
    */
}

pub type ObjectArchiveMetaForUniBackup = ObjectArchiveMeta<ObjectArchiveUniMeta>;
pub type ObjectArchiveMetaForStateBackup = ObjectArchiveMeta<ObjectArchiveStateMeta>;
