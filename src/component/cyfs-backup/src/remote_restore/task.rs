
use cyfs_base::*;
use crate::{archive_download::*, remote_restore::status::RemoteRestoreTaskPhase};
use cyfs_backup_lib::*;
use super::status::*;
use crate::backup::RestoreManager;

use std::sync::Arc;

pub struct RemoteRestoreParams {
    // TaskId, should be valid segment string of path
    pub id: String,

    // Restore related params
    pub cyfs_root: String,
    pub isolate: String,
    pub password: Option<ProtectedPassword>,

    // Remote archive info
    pub remote_archive: RemoteArchiveInfo,
}


#[derive(Clone)]
pub struct RemoteRestoreTask {
    id: String,

    status: RemoteRestoreStatusManager,
}

impl RemoteRestoreTask {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
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

        ret
    }

    async fn run_inner(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        let tmp_dir = cyfs_util::get_temp_path().join("restore");
        let archive_dir = tmp_dir.join(&self.id);
        if archive_dir.is_dir() {
            warn!("local archive dir exists already! {}", archive_dir.display());
        } else {
            if let Err(e) = async_std::fs::create_dir_all(&archive_dir).await {
                let msg = format!("create local archive dir failed! {}, {}", archive_dir.display(), e);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        match params.remote_archive {
            RemoteArchiveInfo::ZipFile(file_url) => {
                let url = file_url.parse_url()?;
                let local_file = archive_dir.join("archive");

                // First download archive to local file
                let progress = ArchiveProgressHolder::new();
                self.status.begin_download(progress.clone());

                info!("will download archive to local file: {} -> {}", url, local_file.display());
                
                let downloader = ArchiveFileDownloader::new(url, local_file.clone());
                downloader.download(&progress).await?;

                // Then unpack the archive to target dir
                let progress = ArchiveProgressHolder::new();
                self.status.begin_unpack(progress.clone());

                info!("will extract archive to local dir: {} -> {}", local_file.display(), archive_dir.display());

                let unzip = ArchiveUnzip::new(local_file, archive_dir.clone());
                unzip.unzip(&progress).await?;
            }
            RemoteArchiveInfo::Folder(folder_url) => {
                let progress = ArchiveProgressHolder::new();
                self.status.begin_download(progress.clone());

                info!("will download archive to local dir: {} -> {}", folder_url.base_url, archive_dir.display());
                
                let downloader = ArchiveFolderDownloader::new(folder_url, archive_dir.clone());
                downloader.download(&progress).await?;
            }
        }
        
        // Create restore task
        let restore_manager = Arc::new(RestoreManager::new());
        let restore_params = UniRestoreParams {
            id: params.id,
            cyfs_root: params.cyfs_root,
            isolate: params.isolate,
            archive: archive_dir,
            password: params.password,
        };

        self.status.begin_restore(&restore_params.id, restore_manager.clone());

        restore_manager.run_uni_restore(restore_params).await?;

        Ok(())
    }
}