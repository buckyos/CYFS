use std::path::{Path, PathBuf};
use log::*;
use cyfs_base_meta::*;
use async_trait::async_trait;
use crate::*;

use cyfs_base::*;

use crate::state_storage::StateRef;
use async_std::sync::{Arc, MutexGuard};

pub fn storage_in_mem_path() -> &'static Path {
    static STORAGE_IN_MEM: &str = "inmemory";
    Path::new(STORAGE_IN_MEM)
}

pub type StorageRef = Arc<Box<dyn Storage>>;

#[async_trait]
pub trait Storage: std::marker::Send + Sync {
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
        async_std::task::block_on(async move {
            let state_ref = self.create_state(false).await;
            MetaExtensionManager::init_extension(&state_ref).await
        })?;

        Ok(())
    }

    async fn create_state(&self, read_only: bool) -> StateRef;

    async fn get_locker(&self) -> MutexGuard<'_, ()>;
    // async fn run_in_transaction<Fn>(&self, func: Fn) where Fn: FnOnce(StateRef) -> dyn Future<Output=BuckyResult<()>>;
}

