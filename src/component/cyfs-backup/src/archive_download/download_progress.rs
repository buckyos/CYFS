use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileDownloadProgress {
    pub file: String,
    pub total: u64,
    pub downloaded: u64,
    pub result: Option<BuckyResult<()>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchiveDownloadProgress {
    pub total: u64,
    pub downloaded: u64,
    pub result: Option<BuckyResult<()>>,

    pub current: Option<FileDownloadProgress>,
}

impl ArchiveDownloadProgress {
    pub fn new(total: u64) -> Self {
        Self {
            total,
            downloaded: 0,
            result: None,
            current: None,
        }
    }

    pub fn begin_download_file(&mut self, file: &str, total: u64) {
        // For single file archive mode
        if self.total == 0 {
            self.total = total;
        }

        let file_progress = FileDownloadProgress {
            file: file.to_owned(),
            total,
            downloaded: 0,
            result: None,
        };

        self.current = Some(file_progress);
    }

    pub fn update_current_file_progress(&mut self, downloaded: u64) {
        assert!(self.current.is_some());
        self.current.as_mut().unwrap().downloaded = downloaded;
        self.downloaded += downloaded;
    }

    pub fn finish_current_file(&mut self, result: BuckyResult<()>) {
        assert!(self.current.is_some());
        self.current.as_mut().unwrap().result = Some(result);
        self.current = None;
    }
}


#[derive(Clone)]
pub struct ArchiveDownloadProgessHolder(Arc<Mutex<ArchiveDownloadProgress>>);

impl ArchiveDownloadProgessHolder {
    pub fn new(total: u64) -> Self {
        Self(Arc::new(Mutex::new(ArchiveDownloadProgress::new(total))))
    }

    pub fn begin_download_file(&self, file: &str, total: u64) {
        self.0.lock().unwrap().begin_download_file(file, total)
    }

    pub fn update_current_file_progress(&self, downloaded: u64) {
        self.0.lock().unwrap().update_current_file_progress(downloaded)
    }

    pub fn finish_current_file(&self, result: BuckyResult<()>) {
        self.0.lock().unwrap().finish_current_file(result)
    }
}
