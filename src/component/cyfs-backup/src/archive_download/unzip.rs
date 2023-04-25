use cyfs_base::*;
use super::progress::ArchiveProgessHolder;

use std::path::PathBuf;

pub struct ArchiveUnzip {
    archive_file: PathBuf,
    target_folder: PathBuf,
}

impl ArchiveUnzip {
    pub async fn unzip(&self) -> BuckyResult<()> {
        self.unzip_inner().await
    }

    async fn unzip_inner(&self) -> BuckyResult<()> {
        let file = std::fs::File::open(&self.archive_file).map_err(|e| {
            let msg = format!(
                "open archive file failed! file={}, {}",
                self.archive_file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let total = file.metadata().map_err(|e| {
            let msg = format!(
                "get metadata from archive file failed! file={}, {}",
                self.archive_file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?.len();


        let mut archive = zip::ZipArchive::new(file).map_err(|e| {
            let msg = format!(
                "open archive file failed! file={}, {}",
                self.archive_file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                let msg = format!("load file from archive failed! index={}, {}", i, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            #[allow(deprecated)]
            let file_path = file.sanitized_name();

            #[allow(deprecated)]
            let file_path = self.target_folder.join(file_path);

            if file.is_dir() {
                std::fs::create_dir_all(&file_path).map_err(|e| {
                    let msg = format!(
                        "create archive inner dir failed! file={}, {}",
                        file_path.display(),
                        e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;
            } else {
                if let Some(dir) = file_path.parent() {
                    if !dir.is_dir() {
                        async_std::fs::create_dir_all(&dir).await.map_err(|e| {
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

                let mut out = std::fs::File::create(&file_path).map_err(|e| {
                    let msg = format!(
                        "create local archive file failed! dir={}, {}",
                        file_path.display(),
                        e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;

                std::io::copy(&mut file, &mut out).map_err(|e| {
                    let msg = format!(
                        "extract archive file to local file failed! file={}, {}",
                        file_path.display(),
                        e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;
            }
        }

        Ok(())
    }
}