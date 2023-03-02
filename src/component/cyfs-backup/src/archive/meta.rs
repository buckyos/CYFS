use crate::object_pack::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::path::Path;


#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveDataMeta {
    pub count: u64,
    pub bytes: u64,
}

impl Default for ObjectArchiveDataMeta {
    fn default() -> Self {
        Self { count: 0, bytes: 0 }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveDataMetas {
    pub objects: ObjectArchiveDataMeta,
    pub chunks: ObjectArchiveDataMeta,
}

impl Default for ObjectArchiveDataMetas {
    fn default() -> Self {
        Self {
            objects: ObjectArchiveDataMeta::default(),
            chunks: ObjectArchiveDataMeta::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveDecMeta {
    pub dec_id: ObjectId,
    pub dec_root: ObjectId,

    pub data: ObjectArchiveDataMetas,
    pub missing: ObjectArchiveDataMetas,
    pub error: ObjectArchiveDataMetas,
}

impl ObjectArchiveDecMeta {
    pub fn new(dec_id: ObjectId,dec_root: ObjectId,) -> Self {
        Self {
            dec_id,
            dec_root,

            data: ObjectArchiveDataMetas::default(),
            missing: ObjectArchiveDataMetas::default(),
            error: ObjectArchiveDataMetas::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveIsolateMeta {
    isolate_id: ObjectId,
    decs: Vec<ObjectArchiveDecMeta>,
    root: ObjectId,
    revision: u64,

    data: ObjectArchiveDataMetas,
    missing: ObjectArchiveDataMetas,
    error: ObjectArchiveDataMetas,
}

impl ObjectArchiveIsolateMeta {
    pub fn new(isolate_id: ObjectId, root: ObjectId, revision: u64) -> Self {
        Self {
            isolate_id,
            decs: vec![],
            root,
            revision,

            data: ObjectArchiveDataMetas::default(),
            missing: ObjectArchiveDataMetas::default(),
            error: ObjectArchiveDataMetas::default(),
        }
    }

    pub fn add_dec(&mut self, dec_meta: ObjectArchiveDecMeta) {
        assert!(self
            .decs
            .iter()
            .find(|item| item.dec_id == dec_meta.dec_id)
            .is_none());

        self.decs.push(dec_meta);
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

    pub fn add_isolate_dec(&mut self, isolate_id: &ObjectId, dec_meta: ObjectArchiveDecMeta) {
        let ret = self
            .isolates
            .iter_mut()
            .find(|item| item.isolate_id == *isolate_id);
        match ret {
            Some(item) => {
                item.add_dec(dec_meta);
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
