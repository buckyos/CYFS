use super::file_meta::ArchiveInnerFileMeta;
use super::meta::*;
use super::verifier::*;
use crate::object_pack::*;
use cyfs_base::*;

use async_std::io::Read as AsyncRead;
use std::path::PathBuf;

pub struct ObjectArchiveInnerFile {
    pub data: Box<dyn AsyncRead + Unpin + Sync + Send + 'static>,
    pub meta: Option<ArchiveInnerFileMeta>,
}

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

        let object_reader =
            ObjectPackSerializeReader::new(meta.format, root.clone(), meta.object_files.clone());
        let chunk_reader =
            ObjectPackSerializeReader::new(meta.format, root.clone(), meta.chunk_files.clone());
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

    pub async fn verify(&self) -> BuckyResult<ObjectArchiveVerifyResult> {
        ObjectArchiveVerifier::new(self.root.clone())
            .verify(&self.meta)
            .await
    }

    pub fn reset_object(&mut self) {
        self.object_reader.reset()
    }

    pub async fn next_object(&mut self) -> BuckyResult<Option<(ObjectId, ObjectArchiveInnerFile)>> {
        let ret = self.object_reader.next_data().await?;
        Self::convert(ret)
    }

    pub fn reset_chunk(&mut self) {
        self.chunk_reader.reset()
    }

    pub async fn next_chunk(&mut self) -> BuckyResult<Option<(ObjectId, ObjectArchiveInnerFile)>> {
        let ret = self.chunk_reader.next_data().await?;
        Self::convert(ret)
    }

    fn convert(
        info: Option<(ObjectId, ObjectPackInnerFile)>,
    ) -> BuckyResult<Option<(ObjectId, ObjectArchiveInnerFile)>> {
        if info.is_none() {
            return Ok(None);
        }

        let (object_id, info) = info.unwrap();
        let meta = match info.meta {
            Some(data) => Some(ArchiveInnerFileMeta::clone_from_slice(&data).map_err(|e| {
                let msg = format!(
                    "decode archive file meta failed! object={}, {}",
                    object_id, e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?),
            None => None,
        };

        let info = ObjectArchiveInnerFile {
            data: info.data,
            meta,
        };

        Ok(Some((object_id, info)))
    }
}
