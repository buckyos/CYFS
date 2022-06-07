use std::path::{Path, PathBuf};
use log::*;
use std::fs::{create_dir};
use cyfs_base::*;
use cyfs_base_meta::*;

use crate::state_storage::StorageRef;

pub struct Snapshot {
    block_hash: BlockHash,
    path: PathBuf
}

impl Snapshot {
    pub fn block_hash(&self) -> BlockHash {
        self.block_hash
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn remove(&self) -> BuckyResult<()> {
        std::fs::remove_file(self.path()).map_err(|_| crate::meta_err!(ERROR_NOT_FOUND))
    }

    pub fn exists(&self) -> bool {
        self.path.as_path().exists()
    }
}

pub struct SnapshotManager {
    dir: PathBuf
}


impl SnapshotManager {
    pub fn new(dir: PathBuf) -> BuckyResult<Self> {
        if !dir.exists() {
            create_dir(dir.as_path()).unwrap();
        }

        Ok(SnapshotManager {
            dir: dir
        })
    }

    fn snapshot_path(&self, block_hash: &BlockHash) -> PathBuf {
        self.dir.join(block_hash.to_hex().unwrap())
    }

    pub fn create_snapshot(&self, block_hash: &BlockHash, storage: &StorageRef) -> BuckyResult<Snapshot> {
        let snapshot = Snapshot {
            block_hash: *block_hash,
            path: self.snapshot_path(block_hash)
        };
        std::fs::copy(storage.path(), snapshot.path()).map_err(|err| {
            error!("create snapshot {} from {} failed, err {}", snapshot.path().display(), storage.path().display(), err);
            crate::meta_err!(ERROR_NOT_FOUND)})?;
        Ok(snapshot)
    }

    pub async fn get_snapshot(&self, block_hash: &BlockHash) -> BuckyResult<Snapshot> {
        let snapshot = Snapshot {
            block_hash: *block_hash,
            path: self.snapshot_path(block_hash)
        };
        if snapshot.exists() {
            Ok(snapshot)
        } else {
            Err(crate::meta_err!(ERROR_NOT_FOUND))
        }
    }
}
