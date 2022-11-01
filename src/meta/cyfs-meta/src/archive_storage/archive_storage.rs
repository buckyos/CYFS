use std::path::{Path, PathBuf};
use log::*;
use cyfs_base_meta::*;
use async_trait::async_trait;

use cyfs_base::*;

use crate::{archive_storage::ArchiveRef, meta_err};
use async_std::sync::{Arc, MutexGuard};

pub fn storage_in_mem_path() -> &'static Path {
    static STORAGE_IN_MEM: &str = "inmemory";
    Path::new(STORAGE_IN_MEM)
}

pub type ArchiveStorageRef = Arc<Box<dyn ArchiveStorage>>;

#[async_trait]
pub trait ArchiveStorage: std::marker::Send + Sync {
    fn path(&self) -> &Path;

    async fn state_hash(&self) -> BuckyResult<StateHash>;

    fn remove(&self) -> BuckyResult<()> {
        std::fs::remove_file(self.path()).map_err(|err| crate::meta_err!({
            error!("remove file {} fail, err {}", self.path().display(), err);
            ERROR_NOT_FOUND
        }))
    }

    async fn backup(&self, height: i64) -> BuckyResult<()> {
        let _locker = self.get_locker().await;
        if height > 5 {
            let backup_file = PathBuf::from(format!("{}_{}", self.path().to_str().unwrap(), height - 5));
            if backup_file.exists() {
                let _ = std::fs::remove_file(backup_file);
            }
        }
        let backup_file = format!("{}_{}", self.path().to_str().unwrap(), height);

        std::fs::copy(self.path(), backup_file).map_err(|err| meta_err!({
            error!("backup file {} fail.height {}, err {}", self.path().display(), height, err);
            ERROR_NOT_FOUND
        }))?;
        Ok(())
    }

    fn backup_exist(&self, height: i64) -> bool {
        let backup_file = format!("{}_{}", self.path().to_str().unwrap(), height);
        Path::new(backup_file.as_str()).exists()
    }

    async fn recovery(&self, height: i64) -> BuckyResult<()> {
        {
            let _locker = self.get_locker().await;
            let backup_file = format!("{}_{}", self.path().to_str().unwrap(), height);

            std::fs::copy(backup_file, self.path()).map_err(|err| meta_err!({
            error!("recovery file {} fail.height {}, err {}", self.path().display(), height, err);
            ERROR_NOT_FOUND
        }))?;
        }

        Ok(())
    }

    async fn create_archive(&self, read_only: bool) -> ArchiveRef;

    async fn get_locker(&self) -> MutexGuard<'_, ()>;
}

