use crate::object_pack::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveIsolateMeta {
    isolate_id: ObjectId,
    decs: Vec<ObjectId>,
    root: ObjectId,
    revision: u64,
}

impl ObjectArchiveIsolateMeta {
    pub fn new(isolate_id: ObjectId, root: ObjectId, revision: u64) -> Self {
        Self {
            isolate_id,
            decs: vec![],
            root,
            revision,
        }
    }

    pub fn add_dec(&mut self, dec_id: &ObjectId) {
        if !self.decs.contains(dec_id) {
            self.decs.push(dec_id.to_owned());
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveMeta {
    pub format: ObjectPackFormat,
    pub isolates: Vec<ObjectArchiveIsolateMeta>,
    pub object_files: Vec<ObjectPackFileInfo>,
    pub chunk_files: Vec<ObjectPackFileInfo>,
}

impl ObjectArchiveMeta {
    pub fn new(format: ObjectPackFormat) -> Self {
        Self {
            format,
            isolates: vec![],
            object_files: vec![],
            chunk_files: vec![],
        }
    }

    pub fn add_isolate(&mut self, isolate_id: &ObjectId, root: ObjectId, revision: u64) {
        let ret = self
            .isolates
            .iter()
            .find(|item| item.isolate_id == *isolate_id);
        if ret.is_none() {
            self.isolates.push(ObjectArchiveIsolateMeta::new(
                isolate_id.to_owned(),
                root,
                revision,
            ));
        }
    }

    pub fn add_isolate_dec(&mut self, isolate_id: &ObjectId, dec_id: &ObjectId) {
        let ret = self
            .isolates
            .iter_mut()
            .find(|item| item.isolate_id == *isolate_id);
        match ret {
            Some(item) => {
                item.add_dec(dec_id);
            }
            None => {
                unreachable!();
                // let mut item = ObjectArchiveIsolateMeta::new(isolate_id.to_owned());
                // item.add_dec(dec_id);
                // self.isolates.push(item);
            }
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
