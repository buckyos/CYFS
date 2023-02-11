use super::blob::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub struct FileBlobStorage {
    root: PathBuf,
    #[cfg(target_os = "windows")]
    upgrade: super::old_base36::FileBlobStorageUpgrade,
}

impl FileBlobStorage {
    pub fn new(root: PathBuf) -> Self {
        Self {
            #[cfg(target_os = "windows")]
            upgrade: super::old_base36::FileBlobStorageUpgrade::new(root.clone()),

            root,
        }
    }

    async fn get_full_path(&self, object_id: &ObjectId, auto_create: bool) -> BuckyResult<PathBuf> {
        let hash_str;
        let len;
        #[cfg(target_os = "windows")]
        {
            hash_str = object_id.to_base36();
            len = 3;
        }
        #[cfg(not(target_os = "windows"))]
        {
            hash_str = object_id.to_string();
            len = 2;
        }

        let (tmp, first) = hash_str.split_at(hash_str.len() - len);
        let second = tmp.split_at(tmp.len() - len).1;

        /* Do not use the following reserved names as filenames: CON、PRN、AUX、NUL、COM1、COM2、COM3、COM4、COM5、COM6、COM7、COM8、COM9、LPT1、LPT2、LPT3、LPT4、LPT5、 LPT6、LPT7、LPT8、 LPT9 */
        #[cfg(target_os = "windows")]
        let second = match second {
            "con" | "aux" | "nul" | "prn" => tmp.split_at(tmp.len() - (len + 1)).1,
            _ => second,
        };

        #[cfg(target_os = "windows")]
        let first = match first {
            "con" | "aux" | "nul" | "prn" => Cow::Owned(format!("{}_", first)),
            _ => Cow::Borrowed(first),
        };

        let path = self.root.join(format!("{}/{}", first, second));
        if auto_create && !path.exists() {
            async_std::fs::create_dir_all(&path).await.map_err(|e| {
                let msg = format!(
                    "create dir for object blob error! path={}, {}",
                    path.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
        }

        let path = path.join(hash_str);

        Ok(path)
    }

    async fn load_object(&self, path: &Path) -> BuckyResult<NONObjectInfo> {
        let object_raw = async_std::fs::read(&path).await.map_err(|e| {
            let msg = format!(
                "read object blob from file error! path={}, {}",
                path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let info = NONObjectInfo::new_from_object_raw(object_raw)?;
        Ok(info)
    }

    fn write_sync<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        fn inner(path: &Path, contents: &[u8]) -> std::io::Result<()> {
            let mut file = File::create(path)?;
            file.write_all(contents)?;
            file.sync_all()?;
            Ok(())
        }
        inner(path.as_ref(), contents.as_ref())
    }

    async fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> std::io::Result<()> {
        let path = path.as_ref().to_owned();
        let contents = contents.as_ref().to_owned();
        async_std::task::spawn_blocking(move || Self::write_sync(&path, contents)).await
    }
}

#[async_trait::async_trait]
impl BlobStorage for FileBlobStorage {
    async fn put_object(&self, data: NONObjectInfo) -> BuckyResult<()> {
        let path = self.get_full_path(&data.object_id, true).await?;

        Self::write(&path, &data.object_raw).await.map_err(|e| {
            let msg = format!(
                "save object blob to file error! path={}, size={}bytes, {}",
                path.display(),
                data.object_raw.len(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!(
            "save object blob to file success! object={}, size={}bytes",
            data.object_id,
            data.object_raw.len(),
        );
        Ok(())
    }

    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<NONObjectInfo>> {
        let path = self.get_full_path(object_id, false).await?;
        if !path.exists() {
            #[cfg(target_os = "windows")]
            {
                if !self.upgrade.try_update(&path, object_id) {
                    return Ok(None);
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                return Ok(None);
            }
        }

        let info = self.load_object(&path).await?;

        Ok(Some(info))
    }

    async fn delete_object(
        &self,
        object_id: &ObjectId,
        flags: u32,
    ) -> BuckyResult<BlobStorageDeleteObjectResponse> {
        let path = self.get_full_path(object_id, false).await?;
        if !path.exists() {
            let resp = BlobStorageDeleteObjectResponse {
                delete_count: 0,
                object: None,
            };

            return Ok(resp);
        }

        let object = if flags & CYFS_NOC_FLAG_DELETE_WITH_QUERY != 0 {
            match self.load_object(&path).await {
                Ok(info) => Some(info),
                Err(_) => {
                    // FIXME what to do if load error when delete object?
                    None
                }
            }
        } else {
            None
        };

        async_std::fs::remove_file(&path).await.map_err(|e| {
            let msg = format!(
                "remove object blob file error! path={}, {}",
                path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!("remove object blob file success! object={}", object_id);

        let resp = BlobStorageDeleteObjectResponse {
            delete_count: 1,
            object,
        };

        Ok(resp)
    }

    async fn exists_object(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        let path = self.get_full_path(object_id, false).await?;
        Ok(path.exists())
    }

    async fn stat(&self) -> BuckyResult<BlobStorageStat> {
        // TODO
        let resp = BlobStorageStat {
            count: 0,
            storage_size: 0,
        };

        Ok(resp)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cyfs_core::*;
    use cyfs_util::get_cyfs_root_path;

    async fn test_dir() {
        let root = get_cyfs_root_path().join("tmp").join("test_blob_storage");
        std::fs::create_dir_all(&root).unwrap();
        let storage = FileBlobStorage::new(root);

        let count: usize = 1024 * 1024;
        for i in 0..count {
            let obj = Text::create(&format!("test{}", i), "", "");
            let id = obj.desc().calculate_id();
            storage.get_full_path(&id, true).await.unwrap();
            if i % 1024 == 0 {
                println!("gen dir index: {}", i);
            }
        }
    }

    #[test]
    fn main() {
        async_std::task::block_on(async move {
            test_dir().await;
        });
    }
}
