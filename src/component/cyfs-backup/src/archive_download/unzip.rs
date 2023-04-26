use super::progress::ArchiveProgressHolder;
use cyfs_base::*;
use super::helper::TaskAbortHandler;

use std::io::prelude::*;
use std::path::{Path, PathBuf};


#[derive(Clone)]
pub struct ArchiveUnzip {
    archive_file: PathBuf,
    target_folder: PathBuf,
}

impl ArchiveUnzip {
    pub fn new(archive_file: PathBuf, target_folder: PathBuf) -> Self {
        Self {
            archive_file,
            target_folder,
        }
    }

    pub async fn unzip(&self, progress: &ArchiveProgressHolder, abort_handler: &TaskAbortHandler) -> BuckyResult<()> {
        let this = self.clone(); 
        let progress = progress.clone();
        let abort_handler = abort_handler.clone();
        
        let task = async_std::task::spawn_blocking(move || {
            this.unzip_inner(&progress, abort_handler)
        });

        task.await
    }

    fn unzip_inner(&self, progress: &ArchiveProgressHolder, abort_handler: TaskAbortHandler) -> BuckyResult<()> {
        let file = std::fs::File::open(&self.archive_file).map_err(|e| {
            let msg = format!(
                "open archive file failed! file={}, {}",
                self.archive_file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        // Stat total progress with compressed size
        let total = file
            .metadata()
            .map_err(|e| {
                let msg = format!(
                    "get metadata from archive file failed! file={}, {}",
                    self.archive_file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?
            .len();

        progress.reset_total(total);

        let mut archive = zip::ZipArchive::new(file).map_err(|e| {
            let msg = format!(
                "open archive file failed! file={}, {}",
                self.archive_file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        abort_handler.check_aborted()?;
 
        for i in 0..archive.len() {
            abort_handler.check_aborted()?;

            let mut file = archive.by_index(i).map_err(|e| {
                let msg = format!("load file from archive failed! index={}, {}", i, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            #[allow(deprecated)]
            let target_file_path = self.target_folder.join(file.sanitized_name());

            if file.is_dir() {
                std::fs::create_dir_all(&target_file_path).map_err(|e| {
                    let msg = format!(
                        "create archive inner dir failed! file={}, {}",
                        target_file_path.display(),
                        e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;
            } else {
                #[allow(deprecated)]
                let file_path_str = file
                    .sanitized_name()
                    .as_os_str()
                    .to_string_lossy()
                    .to_string();

                if file.size() == 0 {
                    warn!("got zero byte file! {}", file_path_str);
                    continue;
                }

                // Stat current file use compressed size
                progress.begin_file(&file_path_str, file.compressed_size());

                let ret = self
                    .unzip_file(&mut file, &target_file_path, progress, &abort_handler);
                progress.finish_current_file(ret.clone());

                ret?;
            }
        }

        Ok(())
    }

    fn unzip_file(
        &self,
        zip_file: &mut zip::read::ZipFile<'_>,
        target_file_path: &Path,
        progress: &ArchiveProgressHolder,
        abort_handler: &TaskAbortHandler,
    ) -> BuckyResult<()> {
        if let Some(dir) = target_file_path.parent() {
            if !dir.is_dir() {
                std::fs::create_dir_all(&dir).map_err(|e| {
                    let msg = format!(
                        "create local archive dir failed! dir={}, {}",
                        dir.display(),
                        e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;
            }
        }

        let mut out = std::fs::File::create(&target_file_path).map_err(|e| {
            let msg = format!(
                "create local archive file failed! dir={}, {}",
                target_file_path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        #[allow(deprecated)]
        let file_path_str = zip_file
            .sanitized_name()
            .as_os_str()
            .to_string_lossy()
            .to_string();

        let mut buf = vec![0; 1024 * 64];
        loop {
            abort_handler.check_aborted()?;

            let len: usize = zip_file.read(&mut buf).map_err(|e| {
                let msg = format!(
                    "read data from archive failed! file={}, {}",
                    file_path_str, e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

            if len == 0 {
                break;
            }

            abort_handler.check_aborted()?;

            out.write_all(&buf[..len]).map_err(|e| {
                let msg = format!(
                    "write buf to local archive file failed! file={}, {}",
                    target_file_path.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

            // Estimate a size based on compression ratio
            let mut compress_len = len as u64 * zip_file.compressed_size() / zip_file.size();
            if compress_len > zip_file.compressed_size() {
                compress_len = zip_file.compressed_size();
            }
            progress.update_current_file_progress(compress_len);
        }

        abort_handler.check_aborted()?;

        out.flush().map_err(|e| {
            let msg = format!(
                "flush to local archive file failed! file={}, {}",
                target_file_path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        progress.update_current_file_progress(zip_file.compressed_size());
        /*
        std::io::copy(&mut file, &mut out).map_err(|e| {
            let msg = format!(
                "extract archive file to local file failed! file={}, {}",
                file_path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;
        */

        Ok(())
    }
}
