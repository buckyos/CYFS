use super::meta::*;
use crate::object_pack::{ObjectPackFormat, ObjectPackInnerFile, ObjectPackSerializeReader};
use cyfs_base::*;

use std::path::PathBuf;

pub struct ObjectArchiveSerializeLoader {
    root: PathBuf,
    meta: ObjectArchiveMeta,

    object_reader: ObjectPackSerializeReader,
    chunk_reader: ObjectPackSerializeReader,
}

impl ObjectArchiveSerializeLoader {
    pub async fn load(root: PathBuf) -> BuckyResult<Self> {
        if !root.is_dir() {
            let msg = format!("invalid object archive root dir: {}", root.display());
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        // First load index into meta
        let meta_file = root.join("index");
        let meta = ObjectArchiveMeta::load(&meta_file).await?;

        let object_reader = ObjectPackSerializeReader::new(
            ObjectPackFormat::Zip,
            root.clone(),
            meta.object_files.clone(),
        );
        let chunk_reader = ObjectPackSerializeReader::new(
            ObjectPackFormat::Zip,
            root.clone(),
            meta.chunk_files.clone(),
        );
        let ret = Self {
            root,
            meta,
            object_reader,
            chunk_reader,
        };

        Ok(ret)
    }

    pub fn meta(&self) -> &ObjectArchiveMeta {
        &self.meta
    }

    pub fn reset_object(&mut self) {
        self.object_reader.reset()
    }

    pub async fn next_object(&mut self) -> BuckyResult<Option<(ObjectId, ObjectPackInnerFile)>> {
        self.object_reader.next_data().await
    }

    pub fn reset_chunk(&mut self) {
        self.chunk_reader.reset()
    }

    pub async fn next_chunk(&mut self) -> BuckyResult<Option<(ObjectId, ObjectPackInnerFile)>> {
        self.chunk_reader.next_data().await
    }
}
