use super::status::*;
use crate::archive_download::TaskAbortHandler;
use crate::archive_download::*;
use crate::backup::RestoreManager;
use cyfs_backup_lib::*;
use cyfs_base::*;

use futures::future::{AbortHandle, Abortable};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct RemoteRestoreTask {
    id: String,

    archive_dir: PathBuf,
    archive_file: PathBuf,

    status: RemoteRestoreStatusManager,

    // Use for cancel task
    task_handle: Arc<Mutex<Option<AbortHandle>>>,
    abort_handler: TaskAbortHandler,
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

            task_handle: Arc::new(Mutex::new(None)),
            abort_handler: TaskAbortHandler::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn status(&self) -> RemoteRestoreStatus {
        self.status.status()
    }

    pub async fn abort(&self) {
        warn!("will cancel restore task: {}", self.id);

        self.abort_handler.abort();

        if let Some(task_handle) = self.task_handle.lock().unwrap().take() {
            task_handle.abort();
        }

        // Wait for task compelte(for some blocking task)
        async_std::task::sleep(std::time::Duration::from_secs(5)).await;

        warn!(
            "cancel restore task complete, now will clean temp data! {}",
            self.id
        );

        self.clean_temp_data().await;
    }

    pub async fn run(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        assert_eq!(self.status.status().phase, RemoteRestoreTaskPhase::Init);

        let (abort_handle, abort_registration) = AbortHandle::new_pair();

        let task = self.clone();
        let fut = Abortable::new(
            async move {
                let ret = task.run_inner(params).await;
                task.status.complete(ret.clone());

                // FIXME should we clean the archive file and archive dir?
                task.clean_temp_data().await;

                ret
            },
            abort_registration,
        );

        {
            let mut holder = self.task_handle.lock().unwrap();
            assert!(holder.is_none());
            *holder = Some(abort_handle);
        }

        match fut.await {
            Ok(ret) => ret,
            Err(futures::future::Aborted { .. }) => {
                let msg = format!("The restore task has been aborted! {}", self.id);
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::Aborted, msg))
            }
        }
    }

    pub fn start(&mut self, params: RemoteRestoreParams) -> BuckyResult<()> {
        let task = self.clone();
        async_std::task::spawn(async move {
            let id = params.id.clone();
            match task.run(params).await {
                Ok(()) => {
                    info!("run remote restore task complete! task={}", id);
                }
                Err(e) => {
                    error!("run remote restore task failed! task={}, {}", id, e);
                }
            }
        });

        Ok(())
    }

    async fn run_inner(&self, params: RemoteRestoreParams) -> BuckyResult<()> {
        let remote_archive = RemoteArchiveInfo::parse(&params.remote_archive).map_err(|e| {
            let msg = format!(
                "invalid remote archive url format: {}, {}",
                params.remote_archive, e
            );
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
                let ret = unzip.unzip(&progress, &self.abort_handler).await;

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
