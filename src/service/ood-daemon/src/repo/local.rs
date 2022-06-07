use super::manager::{Repo, RepoPackageInfo, RepoType};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};

use async_trait::async_trait;
use std::path::{Path, PathBuf};

pub struct LocalRepo {
    local_store: PathBuf,
    ext_list: Vec<&'static str>,
}

impl LocalRepo {
    pub fn new(local_store: Option<String>) -> LocalRepo {
        let path;
        if let Some(v) = local_store {
            path = PathBuf::from(&v);
        } else {
            path = ::cyfs_util::get_cyfs_root_path().join("repo_store");
        }

        info!("will use local repo store: {}", path.display());

        LocalRepo {
            local_store: path,
            ext_list: vec!["", ".zip", ".tar", ".tar.gz"],
        }
    }

    fn find_file(&self, pkg_file_name: &str) -> Option<PathBuf> {
        for ext in &self.ext_list {
            let file_name;
            if ext.len() > 0 {
                file_name = format!("{}{}", pkg_file_name, ext);
            } else {
                file_name = pkg_file_name.to_owned();
            }

            let file_path = self.local_store.join(&file_name);
            debug!("local repo will look for {}", file_path.display());
            if file_path.exists() {
                return Some(file_path);
            }
        }

        None
    }
}

#[async_trait]
impl Repo for LocalRepo {
    fn get_type(&self) -> RepoType {
        return RepoType::Local;
    }

    async fn fetch(&self, info: &RepoPackageInfo, local_file: &Path) -> BuckyResult<()> {
        let full_file_name = if let Some(inner_path) = &info.inner_path {
            format!("{}/{}", info.fid, inner_path)
        } else {
            info.fid.clone()
        };

        let mut ret = self.find_file(&full_file_name);
        if ret.is_none() {
            // 再使用名字查找一遍
            ret = self.find_file(&info.file_name);
            if ret.is_none() {
                let msg = format!(
                    "local package not found! info={:?}, local_store={}",
                    info,
                    self.local_store.display()
                );
                warn!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        }

        let file_path = ret.unwrap();
        info!("got package from local repo: {}", file_path.display());

        let ret = async_std::fs::copy(&file_path, local_file).await;
        if let Err(e) = ret {
            let msg = format!(
                "copy file to target failed! from={}, to={}, err={}",
                file_path.display(),
                local_file.display(),
                e
            );

            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }

        Ok(())
    }
}
