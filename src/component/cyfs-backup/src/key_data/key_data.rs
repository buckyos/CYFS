use crate::meta::KeyDataType;
use cyfs_base::*;

use std::borrow::Cow;
use std::path::{PathBuf, Path};

#[derive(Clone, Debug)]
pub struct KeyData {
    pub local_path: String,
    pub data_type: KeyDataType,
}

impl KeyData {
    pub fn new_file(local_path: impl Into<String>) -> Self {
        Self {
            local_path: Self::fix_path(local_path),
            data_type: KeyDataType::File,
        }
    }

    pub fn new_dir(local_path: impl Into<String>) -> Self {
        Self {
            local_path: Self::fix_path(local_path),
            data_type: KeyDataType::Dir,
        }
    }

    fn fix_path(local_path: impl Into<String>) -> String {
        let local_path: String = local_path.into();
        let local_path = local_path.replace("\\", "/");
        local_path
    }
}

pub struct KeyDataManager {
    cyfs_root: PathBuf,
    list: Vec<KeyData>,
    filter_list: Vec<globset::GlobMatcher>,
}

impl KeyDataManager {
    pub fn new_uni(isolate: &str, filters: &Vec<String>) -> BuckyResult<Self> {
        let mut filter_list = vec![];
        for filter in filters {
            let glob = globset::GlobBuilder::new(filter)
                .case_insensitive(true)
                .literal_separator(true)
                .build()
                .map_err(|e| {
                    let msg = format!(
                        "parse key data filter as glob error! token={}, {}",
                        filter, e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

            filter_list.push(glob.compile_matcher());
        }

        let mut list = vec![];
        let data = if isolate.is_empty() {
            KeyData::new_dir("etc")
        } else {
            KeyData::new_dir(format!("etc/{}", isolate))
        };

        list.push(data);

        let data_dir = if isolate.is_empty() {
            Cow::Borrowed("data")
        } else {
            Cow::Owned(format!("data/{}", isolate))
        };

        let data = KeyData::new_file(format!("{}/named-object-cache/meta.db", data_dir));
        list.push(data);

        let data = KeyData::new_file(format!("{}/named-data-cache/data.db", data_dir));
        list.push(data);

        let data = KeyData::new_file(format!("{}/task-manager/data.db", data_dir));
        list.push(data);

        let data = KeyData::new_file(format!("{}/tracker-cache/data.db", data_dir));
        list.push(data);

        let data = KeyData::new_file(format!("{}/tracker-cache/trans.db", data_dir));
        list.push(data);

        let chunk_cache_dir = if isolate.is_empty() {
            "default"
        } else {
            isolate
        };

        let data = KeyData::new_file(format!("data/chunk-cache/{}/cache.meta", chunk_cache_dir));
        list.push(data);

        let cyfs_root = cyfs_util::get_cyfs_root_path();
        let ret = Self {
            cyfs_root,
            list,
            filter_list,
        };

        Ok(ret)
    }

    pub fn cyfs_root(&self) -> &Path {
        &self.cyfs_root
    }
    
    pub fn list(&self) -> &Vec<KeyData> {
        &self.list
    }
    pub fn check_filter(&self, path: &Path) -> bool {
        for filter in &self.filter_list {
            if filter.is_match(path) {
                return false;
            }
        }

        true
    }
}
