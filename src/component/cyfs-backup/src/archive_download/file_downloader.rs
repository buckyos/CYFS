use cyfs_base::*;

use async_std::io::ReadExt;
use async_std::{fs::File, io::WriteExt};
use http_types::Url;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use surf::Client;

pub struct ArchiveDownloader {
    file: PathBuf,
    url: Url,

    total: AtomicU64,
    downloaded: AtomicU64,
}

impl ArchiveDownloader {
    pub fn new(url: Url, file: PathBuf) -> BuckyResult<Self> {
        let ret = Self {
            url,
            file,
            total: AtomicU64::new(0),
            downloaded: AtomicU64::new(0),
        };

        Ok(ret)
    }

    pub async fn download(&self) -> BuckyResult<()> {
        // Get a client instance
        let mut res = Client::new().get(&self.url).await.map_err(|e| {
            let msg = format!(
                "get from remote archive url failed! url={}, {}",
                self.url, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        let content_length = res.len().ok_or_else(|| {
            let msg = format!(
                "get content-length header from remote archive response but not found! url={}",
                self.url
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        self.total.store(content_length as u64, Ordering::SeqCst);

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

        let mut writer = async_std::io::BufWriter::new(file.clone());

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

            writer.write_all(&buf[..len]).await.map_err(|e| {
                let msg = format!(
                    "write buf to local archive file but failed! file={}, {}",
                    self.file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

            self.downloaded.fetch_add(len as u64, Ordering::SeqCst);
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
