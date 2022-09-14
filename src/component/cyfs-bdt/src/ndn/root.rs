use async_std::{
    sync::Arc, 
};
use crate::Timestamp;

use super::{
    download::*,
};

struct RootTaskImpl {
    max_download_speed: u32, 
    download: DownloadGroup, 
}

#[derive(Clone)]
pub struct RootTask(Arc<RootTaskImpl>);

impl RootTask {
    pub fn new(max_download_speed: u32, history_speed: HistorySpeedConfig) -> Self {
        Self(Arc::new(RootTaskImpl {
            max_download_speed, 
            download: DownloadGroup::new(history_speed, None), 
            // upload: RootUploadTask::new(ResourceManager::new(None))
        }))
    }

    // pub fn upload(&self) -> &RootUploadTask {
    //     &self.0.upload
    // }

    pub fn download(&self) -> &DownloadGroup {
        &self.0.download
    }

    pub fn on_schedule(&self, now: Timestamp) {
        self.download().calc_speed(now);
        self.download().on_drain(self.0.max_download_speed);
    }
}


