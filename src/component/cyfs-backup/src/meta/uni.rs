use super::data::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::ops::DerefMut;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveUniMeta {
    pub id: u64,
    pub time: String,

    pub meta: ObjectArchiveDataSeriesMeta,
}

impl ObjectArchiveUniMeta {
    pub fn new(id: u64) -> Self {
        let datetime = chrono::offset::Local::now();
        let time = format!("{:?}", datetime);

        Self {
            id,
            time,
            meta: ObjectArchiveDataSeriesMeta::default(),
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

#[derive(Clone)]
pub struct ObjectArchiveUniMetaHolder {
    meta: Arc<Mutex<ObjectArchiveUniMeta>>,
}

impl ObjectArchiveUniMetaHolder {
    pub fn new(id: u64) -> Self {
        let meta = ObjectArchiveUniMeta::new(id);

        Self {
            meta: Arc::new(Mutex::new(meta)),
        }
    }

    pub fn finish(&self) -> ObjectArchiveUniMeta {
        let meta = {
            let mut meta = self.meta.lock().unwrap();
            let mut empty_meta = ObjectArchiveUniMeta::new(meta.id);
            std::mem::swap(meta.deref_mut(), &mut empty_meta);

            empty_meta
        };

        meta
    }
}
