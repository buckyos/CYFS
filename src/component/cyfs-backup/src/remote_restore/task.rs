use super::def::*;
use super::status::*;
use crate::backup::RestoreManager;
use crate::{archive_download::*, remote_restore::status::RemoteRestoreTaskPhase};
use cyfs_backup_lib::*;
use cyfs_base::*;

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct RemoteRestoreTask {
    id: String,

    archive_dir: PathBuf,
    archive_file: PathBuf,

    status: RemoteRestoreStatusManager,
}

impl RemoteRestoreTask {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();

        let tmp_dir = cyfs_util::get_temp_path().join("restore");
        let archive_dir = tmp_dir.join(&id);
        if archive_dir.is_dir() {
            warn!(
                "local archive dir exists already! {}",
                archive_dir.display()
            );
        }

        let archive_file = archive_dir.join("archive");

        Self {
            id: id.into(),

            archive_dir,
            archive_file,

            status: RemoteRestoreStatusManager::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn status(&self) -> RemoteRestoreStatus {
        self.status.status()
    }

    pub async fn run(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        assert_eq!(self.status.status().phase, RemoteRestoreTaskPhase::Init);

        let ret = self.run_inner(params).await;
        self.status.complete(ret.clone());

        // FIXME should we clean the archive file and archive dir?
        self.clean_temp_data().await;

        ret
    }

    async fn run_inner(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        let remote_archive = RemoteArchiveInfo::parse(&params.remote_archive).map_err(|e| {
            let msg = format!("invalid remote archive url format: {}, {}", params.remote_archive, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        if !self.archive_dir.is_dir() {
            if let Err(e) = async_std::fs::create_dir_all(&self.archive_dir).await {
                let msg = format!(
                    "create local archive dir failed! {}, {}",
                    self.archive_dir.display(),
                    e
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        match remote_archive {
            RemoteArchiveInfo::ZipFile(file_url) => {
                let url = file_url.parse_url()?;

                // First download archive to local file
                let progress = ArchiveProgressHolder::new();
                self.status.begin_download(progress.clone());

                info!(
                    "will download archive to local file: {} -> {}",
                    url,
                    self.archive_file.display()
                );

                let downloader = ArchiveFileDownloader::new(url, self.archive_file.clone());
                downloader.download(&progress).await?;

                // Then unpack the archive to target dir
                let progress = ArchiveProgressHolder::new();
                self.status.begin_unpack(progress.clone());

                info!(
                    "will extract archive to local dir: {} -> {}",
                    self.archive_file.display(),
                    self.archive_dir.display()
                );

                let unzip = ArchiveUnzip::new(self.archive_file.clone(), self.archive_dir.clone());
                let ret = unzip.unzip(&progress).await;

                ret?;
            }
            RemoteArchiveInfo::Folder(folder_url) => {
                let progress = ArchiveProgressHolder::new();
                self.status.begin_download(progress.clone());

                info!(
                    "will download archive to local dir: {} -> {}",
                    folder_url.base_url,
                    self.archive_dir.display()
                );

                let downloader = ArchiveFolderDownloader::new(folder_url, self.archive_dir.clone());
                downloader.download(&progress).await?;
            }
        }

        // Create restore task
        let restore_manager = Arc::new(RestoreManager::new());

        let cyfs_root = params.cyfs_root.unwrap_or(
            cyfs_util::get_cyfs_root_path_ref()
                .as_os_str()
                .to_string_lossy()
                .to_string(),
        );
        let isolate = params.isolate.unwrap_or("".to_owned());

        let restore_params = UniRestoreParams {
            id: params.id,
            cyfs_root,
            isolate,
            archive: self.archive_dir.clone(),
            password: params.password,
        };

        self.status
            .begin_restore(&restore_params.id, restore_manager.clone());

        restore_manager.run_uni_restore(restore_params).await?;

        Ok(())
    }

    async fn clean_temp_data(&self) {
        // Remove the local file after we unpack
        if self.archive_file.is_file() {
            if let Err(e) = async_std::fs::remove_file(&self.archive_file).await {
                error!(
                    "remove temp archive file failed! {}, {}",
                    self.archive_file.display(),
                    e
                );
            } else {
                info!(
                    "remove temp archive file success! {}",
                    self.archive_file.display()
                );
            }
        }

        if self.archive_dir.is_dir() {
            if let Err(e) = async_std::fs::remove_dir_all(&self.archive_dir).await {
                error!(
                    "remove temp archive dir failed! {}, {}",
                    self.archive_dir.display(),
                    e
                );
            } else {
                info!(
                    "remove temp archive dir success! {}",
                    self.archive_dir.display()
                );
            }
        }
    }
}
