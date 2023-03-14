use super::file_meta::ArchiveInnerFileMeta;
use super::index::*;
use super::verifier::*;
use crate::object_pack::*;
use cyfs_base::*;

use std::path::PathBuf;

pub type ObjectArchiveInnerFileData = ObjectPackInnerFileData;

pub struct ObjectArchiveInnerFile {
    pub data: ObjectArchiveInnerFileData,
    pub meta: Option<ArchiveInnerFileMeta>,
}

pub struct ObjectArchiveSerializeLoader {
    root: PathBuf,
    index: ObjectArchiveIndex,

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
        let index = ObjectArchiveIndex::load(&root).await?;

        let object_reader =
            ObjectPackSerializeReader::new(index.format, root.clone(), index.object_files.clone());
        let chunk_reader =
            ObjectPackSerializeReader::new(index.format, root.clone(), index.chunk_files.clone());
        let ret = Self {
            root,
            index,
            object_reader,
            chunk_reader,
        };

        Ok(ret)
    }

    pub fn index(&self) -> &ObjectArchiveIndex {
        &self.index
    }

    pub async fn verify(&self) -> BuckyResult<ObjectArchiveVerifyResult> {
        ObjectArchiveVerifier::new(self.root.clone())
            .verify(&self.index)
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

pub struct ObjectArchiveRandomLoader {
    root: PathBuf,
    index: ObjectArchiveIndex,

    object_reader: ObjectPackRandomReader,
    chunk_reader: ObjectPackRandomReader,
}

impl ObjectArchiveRandomLoader {
    pub async fn load(root: PathBuf) -> BuckyResult<Self> {
        if !root.is_dir() {
            let msg = format!("invalid object archive root dir: {}", root.display());
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        // First load index into meta
        let index = ObjectArchiveIndex::load(&root).await?;

        let mut object_reader =
            ObjectPackRandomReader::new(index.format, root.clone(), index.object_files.clone());
        object_reader.open().await?;

        let mut chunk_reader =
            ObjectPackRandomReader::new(index.format, root.clone(), index.chunk_files.clone());
        chunk_reader.open().await?;

        let ret = Self {
            root,
            index,
            object_reader,
            chunk_reader,
        };

        Ok(ret)
    }

    pub fn index(&self) -> &ObjectArchiveIndex {
        &self.index
    }

    pub async fn verify(&self) -> BuckyResult<ObjectArchiveVerifyResult> {
        ObjectArchiveVerifier::new(self.root.clone())
            .verify(&self.index)
            .await
    }

    pub async fn get_object(
        &mut self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectArchiveInnerFile>> {
        let ret = self.object_reader.get_data(object_id).await?;
        Self::convert(object_id, ret)
    }

    pub async fn get_chunk(
        &mut self,
        chunk_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectArchiveInnerFile>> {
        let ret = self.chunk_reader.get_data(chunk_id).await?;
        Self::convert(chunk_id, ret)
    }

    fn convert(
        object_id: &ObjectId,
        info: Option<ObjectPackInnerFile>,
    ) -> BuckyResult<Option<ObjectArchiveInnerFile>> {
        if info.is_none() {
            return Ok(None);
        }

        let info = info.unwrap();
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

        Ok(Some(info))
    }
}

pub struct ObjectArchiveLoader {
    root: PathBuf,
    random_loader: ObjectArchiveRandomLoader,
    serialize_loader: ObjectArchiveSerializeLoader,
}

impl ObjectArchiveLoader {
    pub async fn load(root: PathBuf) -> BuckyResult<Self> {
        let random_loader = ObjectArchiveRandomLoader::load(root.clone()).await?;
        let serialize_loader = ObjectArchiveSerializeLoader::load(root.clone()).await?;

        random_loader.verify().await.map_err(|e| {
            let msg = format!(
                "verify object archive but failed! root={}, {}",
                root.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })?;

        let ret = Self {
            root,
            random_loader,
            serialize_loader,
        };

        Ok(ret)
    }

    pub fn serialize_reader(&mut self) -> &mut ObjectArchiveSerializeLoader {
        &mut self.serialize_loader
    }

    pub fn random_reader(&mut self) -> &mut ObjectArchiveRandomLoader {
        &mut self.random_loader
    }
}
