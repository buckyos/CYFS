use cyfs_base::*;
use ood_daemon::*;

use std::path::PathBuf;

pub(crate) struct RepoDownloader {
    repo_dir: PathBuf,
}

impl RepoDownloader {
    pub fn new() -> Self {
        Self {
            repo_dir: ::cyfs_util::get_cyfs_root_path().join("repo_store"),
        }
    }

    pub async fn load(&self) -> BuckyResult<()> {
        if !self.repo_dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&self.repo_dir) {
                let msg = format!(
                    "create local repo store dir error! dir={}, {}",
                    self.repo_dir.display(),
                    e
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        let repo_manager = RepoManager::new_with_named_data().await?;

        let list = DEVICE_CONFIG_MANAGER.load_config()?;
        for service_config in list {
            // fid支持dir模式
            let file_name = service_config.fid.replace("/", "_");

            let file = self.repo_dir.join(&file_name);
            if file.exists() {
                info!("local file already exists! file={}", file.display());
                continue;
            }

            let cache_path = repo_manager.fetch_service(&service_config.fid).await?;
            if let Err(e) = async_std::fs::copy(&cache_path, &file).await {
                let msg = format!(
                    "copy file error! {} -> {}, {}",
                    cache_path.display(),
                    file.display(),
                    e
                );
                error!("{}", msg);

                if let Err(e) = async_std::fs::remove_file(&file).await {
                    error!("remove repo file error! file={}, {}", file.display(), e);
                }

                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }

            info!("download repo package success! file={}", file.display());
        }

        let target = self.repo_dir.join("device-config.toml");
        async_std::fs::copy(&PATHS.device_config, &target)
            .await
            .map_err(|e| {
                let msg = format!(
                    "copy device-config.toml error! {} -> {}, {}",
                    PATHS.device_config.display(),
                    target.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        Ok(())
    }
}
