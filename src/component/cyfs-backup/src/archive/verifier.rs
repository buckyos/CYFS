use super::meta::*;
use crate::object_pack::*;
use cyfs_base::*;

use std::path::PathBuf;

pub struct ObjectArchiveVerifier {
    root: PathBuf,
}

pub struct ObjectArchiveFileVerifyResult {
    pub name: String,
    pub result: BuckyResult<()>,
}

pub struct ObjectArchiveFileListVerifyResult {
    pub valid: bool,
    list: Vec<ObjectArchiveFileVerifyResult>,
}

pub struct ObjectArchiveVerifyResult {
    pub valid: bool,
    pub objects: ObjectArchiveFileListVerifyResult,
    pub chunks: ObjectArchiveFileListVerifyResult,
}

impl ObjectArchiveVerifier {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub async fn verify(&self, meta: &ObjectArchiveMeta) -> BuckyResult<ObjectArchiveVerifyResult> {
        let objects = self.verify_file_list(&meta.object_files).await?;
        let chunks = self.verify_file_list(&meta.chunk_files).await?;

        let result = ObjectArchiveVerifyResult {
            valid: objects.valid && chunks.valid,
            objects,
            chunks,
        };

        Ok(result)
    }

    async fn verify_file_list(
        &self,
        file_info_list: &[ObjectPackFileInfo],
    ) -> BuckyResult<ObjectArchiveFileListVerifyResult> {
        let mut result = ObjectArchiveFileListVerifyResult {
            valid: true,
            list: vec![],
        };

        for file_info in file_info_list {
            let ret = self.verify_file(file_info).await?;
            if ret.result.is_err() {
                result.valid = false;
            }

            result.list.push(ret);
        }

        Ok(result)
    }

    async fn verify_file(
        &self,
        file_info: &ObjectPackFileInfo,
    ) -> BuckyResult<ObjectArchiveFileVerifyResult> {
        let mut ret = ObjectArchiveFileVerifyResult {
            name: file_info.name.clone(),
            result: Ok(()),
        };

        let file = self.root.join(&file_info.name);
        if !file.is_file() {
            let msg = format!(
                "object pack file not exists or invalid file! file={}",
                file.display()
            );
            error!("{}", msg);

            ret.result = Err(BuckyError::new(BuckyErrorCode::NotFound, msg));

            return Ok(ret);
        }

        let (hash, len) = cyfs_base::hash_file(&file).await?;
        if len != file_info.file_len {
            let msg = format!(
                "mismatched pack file length, expected={}, got={}, file={}",
                file_info.file_len,
                len,
                file.display()
            );
            error!("{}", msg);

            ret.result = Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));

            return Ok(ret);
        }

        if hash != file_info.hash {
            let msg = format!(
                "mismatched pack file hash, expected={}, got={}, file={}",
                file_info.hash,
                hash,
                file.display()
            );
            error!("{}", msg);

            ret.result = Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));

            return Ok(ret);
        }

        debug!("verify pack file success! file={}", file.display());

        Ok(ret)
    }
}
