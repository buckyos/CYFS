use cyfs_base::{BuckyError, BuckyResult};
use cyfs_util::ZipPackage;

use std::path::PathBuf;
use async_std::prelude::*;

pub(crate) struct LocalPackageManager {
    root: PathBuf,
}

impl LocalPackageManager {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
        }
    }

    pub fn check_package_hash(&self, fid: &str) -> BuckyResult<bool> {
        let current = self.root.join(fid);

        // 检查是否存在.lock文件
        let lock_path = current.join(".lock");
        if lock_path.exists() {
            warn!("dir lock file exists! file={}", lock_path.display());
            return Ok(true);
        }

        // 检查是否存在.hash文件
        let hash_path = current.join(".hash");
        if !hash_path.exists() {
            let msg = format!(
                ".hash not found in service dir! file={}",
                hash_path.display()
            );
            error!("{}", msg);
            return Err(BuckyError::from(msg));
        }

        // 读取hash
        let configed_hash = match std::fs::read_to_string(hash_path.clone()) {
            Ok(hash) => hash,
            Err(e) => {
                let msg = format!("load .hash error! file={}, err={}", hash_path.display(), e);
                error!("{}", msg);
                return Err(BuckyError::from(msg));
            }
        };

        // 计算包的hash
        let mut zip = ZipPackage::new();
        zip.load(&current);

        let dir_hash = match zip.calc_hash() {
            Ok(hash) => hash,
            Err(e) => {
                let msg = format!("calc dir hash error! dir={}, err={}", current.display(), e);
                error!("{}", msg);
                return Err(BuckyError::from(msg));
            }
        };

        if dir_hash != configed_hash {
            return Ok(false);
        }

        Ok(true)
    }

    pub async fn gc(&self, mut reserved_list: Vec<String>) -> BuckyResult<()> {
        let mut entries = async_std::fs::read_dir(&self.root).await.map_err(|e| {
            let msg = format!("read dir error! dir={}, err={}", self.root.display(), e);
            error!("{}", msg);
            BuckyError::from(msg)
        })?;

        // version里面指向的目录一定不可以删除
        let version_file = self.root.join("version");
        let current_fid = async_std::fs::read_to_string(&version_file).await.map_err(|e| {
            let msg = format!("read version file error! file={}, err={}", version_file.display(), e);
            error!("{}", msg);
            BuckyError::from(msg)
        })?;

        let fid = current_fid.trim();
        if !reserved_list.iter().any(|v| v == fid) {
            reserved_list.push(fid.to_owned());
        }
        reserved_list.push("current".to_owned());

        while let Some(res) = entries.next().await {
            let entry = res.map_err(|e| {
                error!("read service dir entry error: {}", e);
                e
            })?;

            let file_name = entry.file_name();
            if reserved_list.iter().any(|v| v.as_str() == file_name) {
                continue;
            }

            let file_path = self.root.join(entry.file_name());
            if !file_path.is_dir() {
                continue;
            }

            info!("will remove service dir: {}", file_path.display());
            //continue;

            if let Err(e) = async_std::fs::remove_dir_all(&file_path).await {
                error!("remove old service dir failed! dir={}, {}", file_path.display(), e);
            } else {
                info!("remove old service dir success! dir={}", file_path.display());
            }
        }

        Ok(())
    }
}
