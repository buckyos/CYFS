use std::path::{Path, PathBuf};
use std::fs::{create_dir};
use cyfs_base::*;
use cyfs_base_meta::*;
use log::*;

use crate::tmp_manager::TmpManager;

use super::state::State;
use super::snapshot_manager::{Snapshot, SnapshotManager};
use crate::state_storage::StorageRef;


pub struct StorageManager {
    dir: PathBuf,
    tmp_manager: TmpManager,
    new_storage: fn (path: &Path) -> StorageRef,
    snapshot_manager: SnapshotManager,
    reserved: Option<fn () -> dyn State>
}

impl StorageManager {
    pub fn new(dir: PathBuf, new_storage: fn (path: &Path) -> StorageRef) -> BuckyResult<Self> {
        if !dir.exists() {
            create_dir(dir.as_path()).unwrap();
        }

        Ok(Self {
            tmp_manager: TmpManager::new(dir.join("storage"))?,
            new_storage: new_storage,
            snapshot_manager: SnapshotManager::new(dir.join("snapshot"))?,
            dir: dir,
            reserved: None
        })
    }

    fn new_empty_storage(&self, path: &Path) -> BuckyResult<StorageRef> {
        let new_storage = self.new_storage;
        let storage = new_storage(path);
        if let Err(e) = storage.remove() {
            warn!("storage at path {} remove fail, err {}", path.display(), e);
        }
        Ok(storage)
    }


    pub fn create_storage(&self, name: &str) -> BuckyResult<StorageRef> {
        let storage = self.new_empty_storage(self.tmp_manager.get_path(Path::new(name)).as_path())?;
        Ok(storage)
    }

    pub fn create_storage_from_storage(&self, name: &str, from: &StorageRef) -> BuckyResult<StorageRef> {
        let storage = self.new_empty_storage(self.tmp_manager.get_path(&Path::new(name)).as_path())?;
        std::fs::copy(from.path(), storage.path()).map_err(|err| {
            error!("copy from {} to {} fail, err {}", from.path().display(), storage.path().display(), err);
            crate::meta_err!(ERROR_NOT_FOUND)})?;
        Ok(storage)
    }

    pub fn create_storage_from_block_hash(&self, name: &str, hash: &BlockHash) -> BuckyResult<StorageRef> {
        let from = async_std::task::block_on(async { self.load_snapshot_from_block_hash(hash).await })?;
        self.create_storage_from_storage(name, &from)
    }

    pub fn save_snapshot(&self, block_hash: &BlockHash, storage: &StorageRef) -> BuckyResult<Snapshot> {
        self.snapshot_manager.create_snapshot(block_hash, storage)
    }

    pub async fn load_snapshot_from_block_hash(&self, hash: &BlockHash) -> BuckyResult<StorageRef> {
        let snapshot = self.snapshot_manager.get_snapshot(hash).await?;
        let new_storage = self.new_storage;
        let storage = new_storage(snapshot.path());
        Ok(storage)
    }

    pub fn load_snapshot(&self, snapshot: &Snapshot) -> BuckyResult<StorageRef> {
        let new_storage = self.new_storage;
        let storage = new_storage(snapshot.path());
        Ok(storage)
    }
}
