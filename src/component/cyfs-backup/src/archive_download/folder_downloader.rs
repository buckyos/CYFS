use crate::archive::ObjectArchiveIndexHelper;
use cyfs_backup_lib::*;
use cyfs_base::*;

use super::def::RemoteArchiveUrl;
use super::file_downloader::ArchiveDownloader;

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

pub struct ArchiveFolderDownloader {
    url_info: RemoteArchiveUrl,
    folder: PathBuf,

    total: AtomicU64,
    downloaded: AtomicU64,
}

impl ArchiveFolderDownloader {
    pub fn new(url_info: RemoteArchiveUrl, folder: PathBuf) -> Self {
        Self {
            url_info,
            folder,

            total: AtomicU64::new(0),
            downloaded: AtomicU64::new(0),
        }
    }

    pub async fn download(&self) -> BuckyResult<()> {
        info!(
            "will download archive index: url={}",
            self.url_info.base_url
        );

        let index = self.download_index().await?;

        info!("download archive index complete: {:?}", index);

        // Sum to got total bytes
        let mut total = 0;
        index.object_files.iter().for_each(|item| {
            total += item.file_len;
        });

        index.chunk_files.iter().for_each(|item| {
            total += item.file_len;
        });

        info!(
            "will download archive data files: url={}, total={}",
            self.url_info.base_url, total
        );

        self.total.store(total, Ordering::SeqCst);

        for item in &index.object_files {
            self.download_file(&index, item).await?;
        }

        for item in &index.chunk_files {
            self.download_file(&index, item).await?;
        }

        info!("download archive complete! total={}", total);

        Ok(())
    }

    async fn download_index(&self) -> BuckyResult<ObjectArchiveIndex> {
        let mut url = self.url_info.clone();
        url.file_name = Some("index".to_owned());
        let url = url.parse_url()?;

        let file = self.folder.join("index");
        let downloader = ArchiveDownloader::new(url, file)?;
        downloader.download().await?;

        ObjectArchiveIndexHelper::load(&self.folder).await
    }

    async fn download_file(
        &self,
        index: &ObjectArchiveIndex,
        item: &ObjectPackFileInfo,
    ) -> BuckyResult<()> {
        let relative_path = match &index.data_folder {
            Some(folder_name) => {
                format!("{}/{}", folder_name, item.name)
            }
            None => item.name.clone(),
        };

        let file = self.folder.join(&relative_path);

        let mut url = self.url_info.clone();
        url.file_name = Some(relative_path);
        let url = url.parse_url()?;

        let downloader = ArchiveDownloader::new(url, file)?;
        downloader.download().await?;

        Ok(())
    }
}
