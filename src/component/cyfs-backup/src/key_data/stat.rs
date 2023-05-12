use crate::meta::*;
use super::key_data::*;


pub struct KeyDataBackupStat {
    key_data_manager: KeyDataManager
}

impl KeyDataBackupStat {
    pub fn new(key_data_manager: KeyDataManager) -> Self {
        Self {
            key_data_manager,
        }
    }

    pub fn stat(&self) -> ObjectArchiveDataMeta {
        let mut result = ObjectArchiveDataMeta::default();

        for item in self.key_data_manager.list() {
            self.stat_data(&mut result, item);
        }

        result
    }

    fn stat_data(&self, result: &mut ObjectArchiveDataMeta, data: &KeyData) {
        let file = self.key_data_manager.cyfs_root().join(&data.local_path);
        if !file.exists() {
            warn!("target key data not exists! {}", file.display());
            return;
        }

        if !self.key_data_manager.check_filter(&file) {
            warn!("key data will be ignored by filter: {}", file.display());
            return;
        }

        match data.data_type {
            KeyDataType::File => {
                result.count += 1;
            },
            KeyDataType::Dir => {
                let walkdir = walkdir::WalkDir::new(file);
                for item in walkdir.into_iter().filter_map(|e| e.ok()) {
                    if !self.key_data_manager.check_filter(&item.path()) {
                        warn!("key data will be ignored by filter: {}", item.path().display());
                        return;
                    }

                    if item.path().is_file() {
                        result.count += 1;
                    }
                }
            }
        }
    }
}
