use super::{file_meta::ArchiveInnerFileMeta, ObjectArchiveIndexHelper};
use cyfs_backup_lib::*;
use crate::object_pack::*;
use cyfs_base::*;

use async_std::io::Read as AsyncRead;
use std::path::PathBuf;

pub struct ObjectArchiveGenerator {
    root: PathBuf,
    index: ObjectArchiveIndex,
    size_limit: u64,

    data_folder: Option<String>,

    object_writer: ObjectPackRollWriter,
    chunk_writer: ObjectPackRollWriter,

    crypto: Option<AesKey>,
}

impl ObjectArchiveGenerator {
    pub fn new(
        id: String,
        format: ObjectPackFormat,
        strategy: ObjectBackupStrategy,
        root: PathBuf,
        data_folder: Option<String>,
        size_limit: u64,
        crypto: Option<AesKey>,
    ) -> Self {
        let object_writer = ObjectPackRollWriter::new(
            format,
            root.clone(),
            ObjectArchiveDataType::Object.as_str(),
            size_limit,
            crypto.clone(),
        );

        let chunk_writer = ObjectPackRollWriter::new(
            format,
            root.clone(),
            ObjectArchiveDataType::Chunk.as_str(),
            size_limit,
            crypto.clone(),
        );

        Self {
            root,
            index: ObjectArchiveIndexHelper::new(id, format, strategy, data_folder.clone()),
            size_limit,
            data_folder,

            object_writer,
            chunk_writer,

            crypto,
        }
    }

    pub fn clone_empty(&self) -> Self {
        Self::new(
            self.index.id.clone(),
            self.index.format,
            self.index.strategy,
            self.root.clone(),
            self.data_folder.clone(),
            self.size_limit,
            self.crypto.clone(),
        )
    }

    pub async fn add_data(
        &mut self,
        object_id: &ObjectId,
        data: Box<dyn AsyncRead + Unpin + Send + Sync + 'static>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<BuckyResult<u64>> {
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
    ) -> BuckyResult<BuckyResult<u64>> {
        let meta_data = Self::encode_meta(object_id, meta)?;

        match object_id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                self.chunk_writer
                    .add_data_buf(object_id, data, meta_data)
                    .await
            }
            _ => {
                self.object_writer
                    .add_data_buf(object_id, data, meta_data)
                    .await
            }
        }
    }

    fn encode_meta(
        object_id: &ObjectId,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<Option<Vec<u8>>> {
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

    pub async fn finish(mut self) -> BuckyResult<ObjectArchiveIndex> {
        self.object_writer.finish().await?;
        self.chunk_writer.finish().await?;

        let object_files = self.object_writer.into_file_list();
        let chunk_files = self.chunk_writer.into_file_list();

        self.index.object_files = object_files;
        self.index.chunk_files = chunk_files;

        Ok(self.index)
    }
}
