use super::meta::*;
use crate::object_pack::{ObjectPackFormat, ObjectPackRollWriter};
use cyfs_base::*;

use async_std::io::Read as AsyncRead;
use std::path::PathBuf;

pub struct ObjectArchiveGenerator {
    root: PathBuf,
    meta: ObjectArchiveMeta,

    object_writer: ObjectPackRollWriter,
    chunk_writer: ObjectPackRollWriter,
}

impl ObjectArchiveGenerator {
    pub fn new(root: PathBuf, size_limit: u64) -> Self {
        let object_writer = ObjectPackRollWriter::new(
            ObjectPackFormat::Zip,
            root.clone(),
            ObjectArchiveDataType::Object.as_str(),
            size_limit,
        );

        let chunk_writer = ObjectPackRollWriter::new(
            ObjectPackFormat::Zip,
            root.clone(),
            ObjectArchiveDataType::Chunk.as_str(),
            size_limit,
        );

        Self {
            root,
            meta: ObjectArchiveMeta::new(),

            object_writer,
            chunk_writer,
        }
    }

    pub async fn add_data(
        &mut self,
        object_id: &ObjectId,
        data: Box<dyn AsyncRead + Unpin + Send + 'static>,
    ) -> BuckyResult<u64> {
        match object_id.obj_type_code() {
            ObjectTypeCode::Chunk => self.chunk_writer.add_data(object_id, data).await,
            _ => self.object_writer.add_data(object_id, data).await,
        }
    }

    pub async fn finish(mut self) -> BuckyResult<ObjectArchiveMeta> {
        self.object_writer.finish().await?;
        self.chunk_writer.finish().await?;

        let object_files = self.object_writer.into_file_list();
        let chunk_files = self.chunk_writer.into_file_list();

        self.meta.object_files = object_files;
        self.meta.chunk_files = chunk_files;

        let meta_file = self.root.join("index");
        self.meta.save(&meta_file).await?;

        Ok(self.meta)
    }
}
