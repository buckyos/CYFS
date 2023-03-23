use super::ObjectArchiveIndexHelper;
use super::file_meta::ArchiveInnerFileMeta;
use super::verifier::*;
use crate::crypto::*;
use crate::object_pack::*;
use cyfs_backup_lib::*;
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
    pub async fn load(
        root: PathBuf,
        index: ObjectArchiveIndex,
        crypto: Option<AesKey>,
    ) -> BuckyResult<Self> {
        if !root.is_dir() {
            let msg = format!("invalid object archive root dir: {}", root.display());
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let data_dir = root.join("data");
        let object_reader = ObjectPackSerializeReader::new(
            index.format,
            data_dir.clone(),
            index.object_files.clone(),
            crypto.clone(),
        );
        let chunk_reader = ObjectPackSerializeReader::new(
            index.format,
            data_dir,
            index.chunk_files.clone(),
            crypto,
        );

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
        let data_dir = self.root.join("data");
        ObjectArchiveVerifier::new(data_dir)
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

    pub async fn next_chunk(&mut self) -> BuckyResult<Option<(ChunkId, ObjectArchiveInnerFile)>> {
        let ret = self.chunk_reader.next_data().await?;
        let ret = Self::convert(ret)?;
        match ret {
            Some((id, data)) => {
                let chunk_id = ChunkId::try_from(&id).map_err(|e| {
                    let msg = format!(
                        "enumerate chunks but the object_id format is invalid! id={}, {}",
                        id, e
                    );
                    error!("{}", msg);
                    BuckyError::new(e.code(), msg)
                })?;

                Ok(Some((chunk_id, data)))
            }
            None => Ok(None),
        }
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
    pub async fn load(
        root: PathBuf,
        index: ObjectArchiveIndex,
        crypto: Option<AesKey>,
    ) -> BuckyResult<Self> {
        if !root.is_dir() {
            let msg = format!("invalid object archive root dir: {}", root.display());
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let data_dir = root.join("data");
        let mut object_reader = ObjectPackRandomReader::new(
            index.format,
            data_dir.clone(),
            index.object_files.clone(),
            crypto.clone(),
        );
        object_reader.open().await?;

        let mut chunk_reader =
            ObjectPackRandomReader::new(index.format, data_dir, index.chunk_files.clone(), crypto);
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
        let data_dir = self.root.join("data");
        ObjectArchiveVerifier::new(data_dir)
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
        chunk_id: &ChunkId,
    ) -> BuckyResult<Option<ObjectArchiveInnerFile>> {
        let ret = self.chunk_reader.get_data(chunk_id.as_object_id()).await?;
        Self::convert(chunk_id.as_object_id(), ret)
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
    pub async fn load(root: PathBuf, password: Option<ProtectedPassword>) -> BuckyResult<Self> {
        // First load index into meta
        let index = ObjectArchiveIndexHelper::load(&root).await?;

        // Check if need password and verify the password
        let crypto = match index.crypto {
            CryptoMode::AES => {
                if password.is_none() {
                    let msg = format!("password required! crypto mode={:?}", index.crypto);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }

                if index.en_device_id.is_none() {
                    let msg = format!("password required but en_device_id field is none!");
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
                }

                let aes_key = AesKeyHelper::gen(password.as_ref().unwrap(), &index.device_id);
                AesKeyHelper::verify_device_id(
                    &aes_key,
                    &index.device_id,
                    index.en_device_id.as_ref().unwrap(),
                )?;
                Some(aes_key)
            }
            CryptoMode::None => None,
        };

        let random_loader =
            ObjectArchiveRandomLoader::load(root.clone(), index.clone(), crypto.clone()).await?;
        let serialize_loader =
            ObjectArchiveSerializeLoader::load(root.clone(), index, crypto).await?;

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
