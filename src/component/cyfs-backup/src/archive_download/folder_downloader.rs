use crate::archive::ObjectArchiveIndexHelper;
use cyfs_backup_lib::*;
use cyfs_base::*;

use super::file_downloader::ArchiveDownloader;
use super::{def::RemoteArchiveUrl, progress::ArchiveProgessHolder};

use std::{
    path::PathBuf,
};

pub struct ArchiveFolderDownloader {
    url_info: RemoteArchiveUrl,
    folder: PathBuf,
}

impl ArchiveFolderDownloader {
    pub fn new(url_info: RemoteArchiveUrl, folder: PathBuf) -> Self {
        Self {
            url_info,
            folder,
        }
    }

    pub async fn download(&self, progress: &ArchiveProgessHolder) -> BuckyResult<()> {
        info!(
            "will download archive index: url={}",
            self.url_info.base_url
        );

        let index = self.download_index(progress).await?;

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

        progress.reset_total(total);

        for item in &index.object_files {
            self.download_file(&index, item, progress).await?;
        }

        for item in &index.chunk_files {
            self.download_file(&index, item, progress).await?;
        }

        info!("download archive complete! total={}", total);

        Ok(())
    }

    async fn download_index(
        &self,
        progress: &ArchiveProgessHolder,
    ) -> BuckyResult<ObjectArchiveIndex> {
        let mut url = self.url_info.clone();
        url.file_name = Some("index".to_owned());
        let url = url.parse_url()?;

        let file = self.folder.join("index");
        let downloader = ArchiveDownloader::new(url, file)?;
        downloader.download(progress).await?;

        ObjectArchiveIndexHelper::load(&self.folder).await
    }

    async fn download_file(
        &self,
        index: &ObjectArchiveIndex,
        item: &ObjectPackFileInfo,
        progress: &ArchiveProgessHolder,
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
        downloader.download(progress).await?;

        Ok(())
    }
}
