use std::path::{Path, PathBuf};
use std::fs::{create_dir};
use cyfs_base::BuckyResult;

pub struct TmpManager {
    dir: PathBuf
}

impl TmpManager {
    pub fn new(dir: PathBuf) -> BuckyResult<Self> {
        if !dir.exists() {
            create_dir(dir.as_path()).unwrap();
        }

        Ok(TmpManager {
            dir
        })
    }

    pub fn get_path(&self, name: &Path) -> PathBuf {
        self.dir.join(name)
    }
}
