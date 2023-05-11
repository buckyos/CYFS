use cyfs_base::*;

use async_std::io::ReadExt;
use async_std::{fs::File, io::WriteExt};
use http_types::Url;
use std::path::PathBuf;
use surf::Client;

use cyfs_backup_lib::ArchiveProgressHolder;

pub struct ArchiveFileDownloader {
    file: PathBuf,
    url: Url,
}

impl ArchiveFileDownloader {
    pub fn new(url: Url, file: PathBuf) -> Self {
        Self { url, file }
    }

    pub async fn download(&self, progress: &ArchiveProgressHolder) -> BuckyResult<()> {
        progress.begin_file(&self.file.as_os_str().to_string_lossy(), 0);

        let ret = self.download_inner(progress).await;
        progress.finish_current_file(ret.clone());

        ret
    }

    async fn download_inner(&self, progress: &ArchiveProgressHolder) -> BuckyResult<()> {
        // Get a client instance
        let mut res = Client::new().get(&self.url).await.map_err(|e| {
            let msg = format!(
                "get from remote archive url failed! url={}, {}",
                self.url, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        if !res.status().is_success() {
            let msg = format!(
                "get from remote archive url failed! url={}, status={}",
                self.url, res.status(),
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        let content_length = res.len().ok_or_else(|| {
            let msg = format!(
                "get content-length header from remote archive response but not found! url={}",
                self.url
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        progress.reset_current_file_total(content_length as u64);

        info!(
            "will download archive file: {} -> {}, len={}bytes",
            self.url,
            self.file.display(),
            content_length
        );

        // Make sure the dir exists
        if let Some(dir) = self.file.parent() {
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

        // Create file and writer instance
        let mut file = File::create(&self.file).await.map_err(|e| {
            let msg = format!(
                "create local archive file for write but failed! file={}, {}",
                self.file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        // Stream download the file with progress
        let mut body = res.take_body().into_reader();

        let mut buf = vec![0; 1024 * 64];
        loop {
            let len: usize = body.read(&mut buf).await.map_err(|e| {
                let msg = format!("read data from remote failed! url={}, {}", self.url, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::ConnectionAborted, msg)
            })?;

            if len == 0 {
                break;
            }

            file.write_all(&buf[..len]).await.map_err(|e| {
                let msg = format!(
                    "write buf to local archive file but failed! file={}, {}",
                    self.file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

            progress.update_current_file_progress(len as u64);
        }

        file.flush().await.map_err(|e| {
            let msg = format!(
                "flush local file error! file={}, {}",
                self.file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!(
            "download archive file complete: {} -> {}, len={}bytes",
            self.url,
            self.file.display(),
            content_length
        );

        Ok(())
    }
}
