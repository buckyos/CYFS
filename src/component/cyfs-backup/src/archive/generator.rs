use super::file_meta::ArchiveInnerFileMeta;
use super::meta::*;
use crate::object_pack::*;
use cyfs_base::*;

use async_std::io::Read as AsyncRead;
use std::path::PathBuf;

pub struct ObjectArchiveGenerator {
    root: PathBuf,
    meta: ObjectArchiveMeta,
    size_limit: u64,

    object_writer: ObjectPackRollWriter,
    chunk_writer: ObjectPackRollWriter,
}

impl ObjectArchiveGenerator {
    pub fn new(id: u64, format: ObjectPackFormat, root: PathBuf, size_limit: u64) -> Self {
        let object_writer = ObjectPackRollWriter::new(
            format,
            root.clone(),
            ObjectArchiveDataType::Object.as_str(),
            size_limit,
        );

        let chunk_writer = ObjectPackRollWriter::new(
            format,
            root.clone(),
            ObjectArchiveDataType::Chunk.as_str(),
            size_limit,
        );

        Self {
            root,
            meta: ObjectArchiveMeta::new(id, format),
            size_limit,

            object_writer,
            chunk_writer,
        }
    }

    pub fn clone_empty(&self) -> Self {
        Self::new(self.meta.id, self.meta.format, self.root.clone(), self.size_limit)
    }
    
    pub fn add_isolate_meta(&mut self, isolate_meta: ObjectArchiveIsolateMeta) {
        self.meta.add_isolate(isolate_meta);
    }
    
    pub async fn add_data(
        &mut self,
        object_id: &ObjectId,
        data: Box<dyn AsyncRead + Unpin + Send + Sync + 'static>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<u64> {
        let meta_data = Self::encode_meta(object_id, meta)?;

        match object_id.obj_type_code() {
            ObjectTypeCode::Chunk => self.chunk_writer.add_data(object_id, data, meta_data).await,
            _ => {
                self.object_writer
                    .add_data(object_id, data, meta_data)
                    .await
            }
        }
    }

    pub async fn add_data_buf(
        &mut self,
        object_id: &ObjectId,
        data: &[u8],
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<u64> {
        let meta_data = Self::encode_meta(object_id, meta)?;

        match object_id.obj_type_code() {
            ObjectTypeCode::Chunk => self.chunk_writer.add_data_buf(object_id, data, meta_data).await,
            _ => {
                self.object_writer
                    .add_data_buf(object_id, data, meta_data)
                    .await
            }
        }
    }

    fn encode_meta(object_id: &ObjectId, meta: Option<ArchiveInnerFileMeta>,) -> BuckyResult<Option<Vec<u8>>> {
        let meta_data = match meta {
            Some(meta) => Some(meta.to_vec().map_err(|e| {
                let msg = format!(
                    "encode archive data failed! object={}, meta={:?}, {}",
                    object_id, meta, e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?),
            None => None,
        };

        Ok(meta_data)
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
