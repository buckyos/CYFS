use crate::meta::*;
use super::key_data::*;

use std::path::PathBuf;

pub struct KeyDataBackupStat {
    cyfs_root: PathBuf,
    list: Vec<KeyData>,
}

impl KeyDataBackupStat {
    pub fn new(keydata: KeyDataManager) -> Self {
        Self {
            cyfs_root: keydata.cyfs_root,
            list: keydata.list,
        }
    }

    pub fn stat(&self) -> ObjectArchiveDataMeta {
        let mut result = ObjectArchiveDataMeta::default();

        for item in &self.list {
            self.stat_data(&mut result, item);
        }

        result
    }

    fn stat_data(&self, result: &mut ObjectArchiveDataMeta, data: &KeyData) {
        let file = self.cyfs_root.join(&data.local_path);
        if !file.exists() {
            warn!("target key data not exists! {}", file.display());
            return;
        }

        match data.data_type {
            KeyDataType::File => {
                result.count += 1;
            },
            KeyDataType::Dir => {
                let walkdir = walkdir::WalkDir::new(file);
                for item in walkdir.into_iter().filter_map(|e| e.ok()) {
                    if item.path().is_file() {
                        result.count += 1;
                    }
                }
            }
        }
    }
}
